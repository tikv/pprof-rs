// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::hash_map::DefaultHasher;
use std::convert::TryInto;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::ManuallyDrop;

use crate::frames::UnresolvedFrames;

use tempfile::NamedTempFile;

pub const BUCKETS: usize = 1 << 12;
pub const BUCKETS_ASSOCIATIVITY: usize = 4;
pub const BUFFER_LENGTH: usize = (1 << 18) / std::mem::size_of::<Entry<UnresolvedFrames>>();

#[derive(Debug)]
pub struct Entry<T> {
    pub item: T,
    pub count: isize,
}

impl<T: Default> Default for Entry<T> {
    fn default() -> Self {
        Entry {
            item: Default::default(),
            count: 0,
        }
    }
}

#[derive(Debug)]
pub struct Bucket<T: 'static> {
    pub length: usize,
    entries: Box<[Entry<T>; BUCKETS_ASSOCIATIVITY]>,
}

impl<T: Eq + Default> Default for Bucket<T> {
    fn default() -> Bucket<T> {
        let entries = Box::default();

        Self { length: 0, entries }
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
            related_bucket: self,
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

pub struct HashCounter<T: Hash + Eq + 'static> {
    buckets: Box<[Bucket<T>; BUCKETS]>,
}

impl<T: Hash + Eq + Default + Debug> Default for HashCounter<T> {
    fn default() -> Self {
        let mut v: Vec<Bucket<T>> = Vec::with_capacity(BUCKETS);
        v.resize_with(BUCKETS, Default::default);
        let buckets = v.into_boxed_slice().try_into().unwrap();

        Self { buckets }
    }
}

impl<T: Hash + Eq> HashCounter<T> {
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
    file: NamedTempFile,
    buffer: Box<[T; BUFFER_LENGTH]>,
    buffer_index: usize,
    flush_n: usize,
}

impl<T: Default + Debug> TempFdArray<T> {
    fn new() -> std::io::Result<TempFdArray<T>> {
        let file = NamedTempFile::new()?;

        let mut v: Vec<T> = Vec::with_capacity(BUFFER_LENGTH);
        v.resize_with(BUFFER_LENGTH, Default::default);
        let buffer = v.into_boxed_slice().try_into().unwrap();

        Ok(Self {
            file,
            buffer,
            buffer_index: 0,
            flush_n: 0,
        })
    }
}

impl<T> TempFdArray<T> {
    fn flush_buffer(&mut self) -> std::io::Result<()> {
        self.buffer_index = 0;
        let buf = unsafe {
            std::slice::from_raw_parts(
                self.buffer.as_ptr() as *const u8,
                BUFFER_LENGTH * std::mem::size_of::<T>(),
            )
        };
        self.flush_n += 1;
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

    fn try_iter<'lt>(
        &'lt self,
        file_buffer_container: &'lt mut Option<Box<[ManuallyDrop<T>]>>,
    ) -> std::io::Result<impl Iterator<Item = &'lt T>> {
        let file_buffer = self.file_buffer()?;
        let file_buffer = file_buffer_container.insert(file_buffer);

        Ok(TempFdArrayIterator {
            buffer: &self.buffer[0..self.buffer_index],
            file_buffer,
            index: 0,
        })
    }

    fn file_buffer(&self) -> std::io::Result<Box<[ManuallyDrop<T>]>> {
        if self.flush_n == 0 {
            return Ok(Vec::new().into_boxed_slice());
        }

        let mut file = self.file.reopen()?;
        file.seek(SeekFrom::Start(0))?;
        let file_buffer = unsafe {
            // Get properly aligned pointer
            let len = BUFFER_LENGTH * self.flush_n;
            // Expect T to be non-ZST
            let layout = std::alloc::Layout::array::<ManuallyDrop<T>>(len).unwrap();
            let ptr = std::alloc::alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            // Populate with bytes
            file.read_exact(std::slice::from_raw_parts_mut(
                ptr,
                len * std::mem::size_of::<T>(),
            ))?;
            // Cast to proper type
            Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                ptr.cast::<ManuallyDrop<T>>(),
                len,
            ))
        };
        file.seek(SeekFrom::End(0))?;

        Ok(file_buffer)
    }
}

pub struct TempFdArrayIterator<'a, T> {
    pub buffer: &'a [T],
    pub file_buffer: &'a [ManuallyDrop<T>],
    pub index: usize,
}

impl<'a, T> Iterator for TempFdArrayIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.buffer.len() {
            self.index += 1;
            Some(&self.buffer[self.index - 1])
        } else if self.index - self.buffer.len() < self.file_buffer.len() {
            self.index += 1;
            Some(&self.file_buffer[self.index - self.buffer.len() - 1])
        } else {
            None
        }
    }
}

pub struct Collector<T: Hash + Eq + 'static> {
    map: HashCounter<T>,
    temp_array: TempFdArray<Entry<T>>,
}

impl<T: Hash + Eq + Default + Debug + 'static> Collector<T> {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            map: HashCounter::<T>::default(),
            temp_array: TempFdArray::<Entry<T>>::new()?,
        })
    }
}

impl<T: Hash + Eq + 'static> Collector<T> {
    pub fn add(&mut self, key: T, count: isize) -> std::io::Result<()> {
        if let Some(evict) = self.map.add(key, count) {
            self.temp_array.push(evict)?;
        }

        Ok(())
    }

    pub fn try_iter<'lt>(
        &'lt self,
        file_buffer_store: &'lt mut Option<Box<[ManuallyDrop<Entry<T>>]>>,
    ) -> std::io::Result<impl Iterator<Item = &'lt Entry<T>>> {
        Ok(self
            .map
            .iter()
            .chain(self.temp_array.try_iter(file_buffer_store)?))
    }
}

#[cfg(test)]
mod test_utils {
    use super::*;
    use std::collections::BTreeMap;

    pub fn add_map(hashmap: &mut BTreeMap<usize, isize>, entry: &Entry<usize>) {
        match hashmap.get_mut(&entry.item) {
            None => {
                hashmap.insert(entry.item, entry.count);
            }
            Some(count) => *count += entry.count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn stack_hash_counter() {
        let mut stack_hash_counter = HashCounter::<usize>::default();
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

    #[test]
    fn evict_test() {
        let mut stack_hash_counter = HashCounter::<usize>::default();
        let mut real_map = BTreeMap::new();

        for item in 0..(1 << 10) * 4 {
            for _ in 0..(item % 4) {
                match stack_hash_counter.add(item, 1) {
                    None => {}
                    Some(evict) => {
                        test_utils::add_map(&mut real_map, &evict);
                    }
                }
            }
        }

        stack_hash_counter.iter().for_each(|entry| {
            test_utils::add_map(&mut real_map, entry);
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

        let mut file_buffer_store = None;
        collector
            .try_iter(&mut file_buffer_store)
            .unwrap()
            .for_each(|entry| {
                test_utils::add_map(&mut real_map, entry);
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
}
