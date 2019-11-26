// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::frames::UnresolvedFrames;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;

pub const BUCKETS: usize = (1 << 12);
pub const BUCKETS_ASSOCIATIVITY: usize = 4;
pub const BUFFER_LENGTH: usize = (1 << 18) / std::mem::size_of::<Entry<UnresolvedFrames>>();

pub struct Entry<T> {
    pub item: T,
    pub count: isize,
}

pub struct Bucket<T: 'static> {
    pub length: usize,
    entries: &'static mut [Entry<T>; BUCKETS_ASSOCIATIVITY],
}

impl<T: Eq> Default for Bucket<T> {
    fn default() -> Bucket<T> {
        let entries = Box::new(unsafe { std::mem::MaybeUninit::uninit().assume_init() });

        Self {
            length: 0,
            entries: Box::leak(entries),
        }
    }
}

impl<T: Eq> Bucket<T> {
    pub fn add(&mut self, key: T, count: isize) -> Option<Entry<T>> {
        let mut done = false;
        self.entries[0..self.length].iter_mut().for_each(|ele| {
            if ele.item == key {
                ele.count += count;
                done = true;
            }
        });

        if done {
            None
        } else if self.length < BUCKETS_ASSOCIATIVITY {
            let ele = &mut self.entries[self.length];
            ele.item = key;
            ele.count = count;

            self.length += 1;
            None
        } else {
            let mut min_index = 0;
            let mut min_count = self.entries[0].count;
            for index in 0..self.length {
                let count = self.entries[index].count;
                if count < min_count {
                    min_index = index;
                    min_count = count;
                }
            }

            let mut new_entry = Entry { item: key, count };
            std::mem::swap(&mut self.entries[min_index], &mut new_entry);
            Some(new_entry)
        }
    }

    pub fn iter(&self) -> BucketIterator<T> {
        BucketIterator::<T> {
            related_bucket: &self,
            index: 0,
        }
    }
}

pub struct BucketIterator<'a, T: 'static> {
    related_bucket: &'a Bucket<T>,
    index: usize,
}

impl<'a, T> Iterator for BucketIterator<'a, T> {
    type Item = &'a Entry<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.related_bucket.length {
            self.index += 1;
            Some(&self.related_bucket.entries[self.index - 1])
        } else {
            None
        }
    }
}

pub struct StackHashCounter<T: Hash + Eq + 'static> {
    buckets: &'static mut [Bucket<T>; BUCKETS],
}

impl<T: Hash + Eq> Default for StackHashCounter<T> {
    fn default() -> Self {
        let buckets = Box::new(unsafe { std::mem::MaybeUninit::uninit().assume_init() });
        let counter = Self {
            buckets: Box::leak(buckets),
        };
        counter.buckets.iter_mut().for_each(|item| {
            *item = Bucket::<T>::default();
        });

        counter
    }
}

impl<T: Hash + Eq> StackHashCounter<T> {
    fn hash(key: &T) -> u64 {
        let mut s = DefaultHasher::new();
        key.hash(&mut s);
        s.finish()
    }

    pub fn add(&mut self, key: T, count: isize) -> Option<Entry<T>> {
        let hash_value = Self::hash(&key);
        let bucket = &mut self.buckets[(hash_value % BUCKETS as u64) as usize];

        bucket.add(key, count)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry<T>> {
        let mut iter: Box<dyn Iterator<Item = &Entry<T>>> =
            Box::new(self.buckets[0].iter().chain(std::iter::empty()));
        for bucket in self.buckets[1..].iter() {
            iter = Box::new(iter.chain(bucket.iter()));
        }

        iter
    }
}

pub struct TempFdArray<T: 'static> {
    file: File,
    buffer: &'static mut [T; BUFFER_LENGTH],
    buffer_index: usize,
    phantom: PhantomData<T>,
}

impl<T> TempFdArray<T> {
    fn new() -> std::io::Result<TempFdArray<T>> {
        let file = tempfile::tempfile()?;
        let buffer = Box::new(unsafe { std::mem::MaybeUninit::uninit().assume_init() });
        Ok(Self {
            file,
            buffer: Box::leak(buffer),
            buffer_index: 0,
            phantom: PhantomData,
        })
    }

    fn flush_buffer(&mut self) -> std::io::Result<()> {
        self.buffer_index = 0;
        let buf = unsafe {
            std::slice::from_raw_parts(
                self.buffer.as_ptr() as *const u8,
                BUFFER_LENGTH * std::mem::size_of::<T>(),
            )
        };
        self.file.write_all(buf)?;

        Ok(())
    }

    fn push(&mut self, entry: T) -> std::io::Result<()> {
        if self.buffer_index >= BUFFER_LENGTH {
            self.flush_buffer()?;
        }

        self.buffer[self.buffer_index] = entry;
        self.buffer_index += 1;

        Ok(())
    }

    fn iter(&mut self) -> std::io::Result<impl Iterator<Item = &T>> {
        let mut file_vec = Vec::new();
        self.file.seek(SeekFrom::Start(0))?;
        self.file.read_to_end(&mut file_vec)?;
        self.file.seek(SeekFrom::End(0))?;

        Ok(TempFdArrayIterator {
            buffer: &self.buffer[0..self.buffer_index],
            file_vec,
            index: 0,
        })
    }
}

pub struct TempFdArrayIterator<'a, T> {
    pub buffer: &'a [T],
    pub file_vec: Vec<u8>,
    pub index: usize,
}

impl<'a, T> Iterator for TempFdArrayIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.buffer.len() {
            self.index += 1;
            Some(&self.buffer[self.index - 1])
        } else {
            let length = self.file_vec.len() / std::mem::size_of::<T>();
            let ts =
                unsafe { std::slice::from_raw_parts(self.file_vec.as_ptr() as *const T, length) };
            if self.index - self.buffer.len() < ts.len() {
                self.index += 1;
                Some(&ts[self.index - self.buffer.len() - 1])
            } else {
                None
            }
        }
    }
}

pub struct Collector<T: Hash + Eq + 'static> {
    map: StackHashCounter<T>,
    temp_array: TempFdArray<Entry<T>>,
}

impl<T: Hash + Eq + 'static> Collector<T> {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            map: StackHashCounter::<T>::default(),
            temp_array: TempFdArray::<Entry<T>>::new()?,
        })
    }

    pub fn add(&mut self, key: T, count: isize) -> std::io::Result<()> {
        if let Some(evict) = self.map.add(key, count) {
            self.temp_array.push(evict)?;
        }

        Ok(())
    }

    pub fn iter(&mut self) -> std::io::Result<impl Iterator<Item = &Entry<T>>> {
        Ok(self.map.iter().chain(self.temp_array.iter()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::ffi::c_void;
    use test::Bencher;

    #[test]
    fn stack_hash_counter() {
        let mut stack_hash_counter = StackHashCounter::<usize>::default();
        stack_hash_counter.add(0, 1);
        stack_hash_counter.add(1, 1);
        stack_hash_counter.add(1, 1);

        stack_hash_counter.iter().for_each(|item| {
            if item.item == 0 {
                assert_eq!(item.count, 1);
            } else if item.item == 1 {
                assert_eq!(item.count, 2);
            } else {
                unreachable!();
            }
        });
    }

    fn add_map(hashmap: &mut BTreeMap<usize, isize>, entry: &Entry<usize>) {
        match hashmap.get_mut(&entry.item) {
            None => {
                hashmap.insert(entry.item, entry.count);
            }
            Some(count) => *count += entry.count,
        }
    }

    #[test]
    fn evict_test() {
        let mut stack_hash_counter = StackHashCounter::<usize>::default();
        let mut real_map = BTreeMap::new();

        for item in 0..(1 << 10) * 4 {
            for _ in 0..(item % 4) {
                match stack_hash_counter.add(item, 1) {
                    None => {}
                    Some(evict) => {
                        add_map(&mut real_map, &evict);
                    }
                }
            }
        }

        stack_hash_counter.iter().for_each(|entry| {
            add_map(&mut real_map, &entry);
        });

        for item in 0..(1 << 10) * 4 {
            let count = (item % 4) as isize;
            match real_map.get(&item) {
                Some(item) => {
                    assert_eq!(*item, count);
                }
                None => {
                    assert_eq!(count, 0);
                }
            }
        }
    }

    #[test]
    fn collector_test() {
        let mut collector = Collector::new().unwrap();
        let mut real_map = BTreeMap::new();

        for item in 0..(1 << 12) * 4 {
            for _ in 0..(item % 4) {
                collector.add(item, 1).unwrap();
            }
        }

        collector.iter().unwrap().for_each(|entry| {
            add_map(&mut real_map, &entry);
        });

        for item in 0..(1 << 12) * 4 {
            let count = (item % 4) as isize;
            match real_map.get(&item) {
                Some(value) => {
                    assert_eq!(count, *value);
                }
                None => {
                    assert_eq!(count, 0);
                }
            }
        }
    }

    extern "C" {
        static mut __malloc_hook: Option<extern "C" fn(size: usize) -> *mut c_void>;

        fn malloc(size: usize) -> *mut c_void;
    }

    thread_local! {
        static FLAG: RefCell<bool> = RefCell::new(false);
    }

    extern "C" fn malloc_hook(size: usize) -> *mut c_void {
        unsafe {
            __malloc_hook = None;
        }

        FLAG.with(|flag| {
            flag.replace(true);
        });
        let p = unsafe { malloc(size) };

        unsafe {
            __malloc_hook = Some(malloc_hook);
        }

        p
    }

    #[test]
    fn malloc_free() {
        let mut collector = Collector::new().unwrap();
        let mut real_map = BTreeMap::new();

        unsafe {
            __malloc_hook = Some(malloc_hook);
        }

        for item in 0..(1 << 10) * 4 {
            for _ in 0..(item % 4) {
                collector.add(item, 1).unwrap();
            }
        }
        unsafe {
            __malloc_hook = None;
        }

        FLAG.with(|flag| {
            assert_eq!(*flag.borrow(), false);
        });

        collector.iter().unwrap().for_each(|entry| {
            add_map(&mut real_map, &entry);
        });

        for item in 0..(1 << 10) * 4 {
            let count = (item % 4) as isize;
            match real_map.get(&item) {
                Some(value) => {
                    assert_eq!(count, *value);
                }
                None => {
                    assert_eq!(count, 0);
                }
            }
        }
    }

    #[bench]
    fn write_into_collector(b: &mut Bencher) {
        let mut collector = Collector::new().unwrap();

        const SIZE: usize = 1000;

        let mut vec: Vec<u64> = Vec::with_capacity(SIZE);
        for _ in 0..vec.capacity() {
            vec.push(rand::random());
        }

        b.iter(|| {
            vec.iter().for_each(|item| {
                collector.add(item.clone(), 1).unwrap();
            });
        });
    }

    #[bench]
    fn write_into_stack_hash_counter(b: &mut Bencher) {
        let mut collector = StackHashCounter::default();

        const SIZE: usize = 1000;

        let mut vec: Vec<u64> = Vec::with_capacity(SIZE);
        for _ in 0..vec.capacity() {
            vec.push(rand::random());
        }

        b.iter(|| {
            vec.iter().for_each(|item| {
                collector.add(item.clone(), 1);
            });
        });
    }
}
