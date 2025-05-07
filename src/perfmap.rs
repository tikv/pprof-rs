use std::{
    io::BufRead,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, RwLock},
    time::Duration,
};

use crate::{Error, Symbol};
use notify_debouncer_mini::{
    new_debouncer,
    notify::{RecursiveMode, Watcher},
    DebounceEventHandler, Debouncer,
};
use once_cell::sync::OnceCell;

#[derive(Debug)]
pub struct PerfMap {
    ranges: Vec<(usize, usize, String)>,
}

impl PerfMap {
    pub fn new(path: &PathBuf) -> Option<Self> {
        let file = std::fs::File::open(path).ok()?;
        let reader = std::io::BufReader::new(file);
        let mut ranges = Vec::new();
        for line in reader.lines() {
            let line = line.ok()?;
            // The format of perf map is:
            // <start addr> <len addr> <name>
            // where <start addr> and <len addr> are hexadecimal numbers.
            // where <name> may contain spaces.
            let mut parts = line.split_whitespace();
            let start = usize::from_str_radix(parts.next()?, 16).ok()?;
            let len = usize::from_str_radix(parts.next()?, 16).ok()?;
            let name = parts.collect::<Vec<_>>().join(" ");
            ranges.push((start, start + len, name));
        }
        Some(Self { ranges })
    }

    pub fn find(&self, addr: usize) -> Option<&str> {
        for (start, end, name) in &self.ranges {
            if *start <= addr && addr < *end {
                return Some(name);
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct PerfMapSymbol(String);

impl From<PerfMapSymbol> for Symbol {
    fn from(value: PerfMapSymbol) -> Self {
        Symbol {
            name: Some(value.0.into_bytes()),
            addr: None,
            filename: None,
            lineno: None,
        }
    }
}

#[derive(Debug)]
pub struct PerfMapResolver {
    perf_map: Arc<RwLock<Option<PerfMap>>>,
}

fn create_debouncer<F: DebounceEventHandler>(
    event_handler: F,
    path: &PathBuf,
) -> Result<Debouncer<impl Watcher>, Error> {
    let mut debouncer =
        new_debouncer(Duration::from_secs(1), event_handler).map_err(|_| Error::CreatingError)?;
    debouncer
        .watcher()
        .watch(path, RecursiveMode::NonRecursive)
        .map_err(|_| Error::CreatingError)?;
    Ok(debouncer)
}

fn touch(path: &PathBuf) -> Result<(), Error> {
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .map_err(|_| Error::CreatingError)?;
    Ok(())
}

impl PerfMapResolver {
    pub fn new() -> Result<Self, Error> {
        let path = PathBuf::from("/tmp/").join(format!("perf-{}.map", std::process::id()));
        touch(&path)?;

        let perf_map = Arc::new(RwLock::new(PerfMap::new(&path)));
        let (tx, rx) = std::sync::mpsc::channel();

        let debouncer = create_debouncer(tx, &path)?;
        let thread_perf_map = Arc::clone(&perf_map);

        std::thread::spawn(move || {
            for result in rx {
                match result {
                    Ok(_events) => {
                        if let Ok(mut perf_map) = thread_perf_map.write() {
                            *perf_map = PerfMap::new(&path);
                        }
                    }
                    _ => {}
                }
            }
            drop(debouncer);
        });
        Ok(Self { perf_map })
    }

    pub fn resolve(&self, addr: usize) -> Option<PerfMapSymbol> {
        if let Ok(Some(perf_map)) = self.perf_map.read().as_deref() {
            perf_map.find(addr).map(|s| PerfMapSymbol(s.to_string()))
        } else {
            None
        }
    }
}

static PERF_MAP_RESOLVER: OnceCell<PerfMapResolver> = OnceCell::new();
static SHOULD_USE_PERF_MAP: AtomicBool = AtomicBool::new(false);

pub fn get_resolver() -> Result<Option<&'static PerfMapResolver>, Error> {
    if SHOULD_USE_PERF_MAP.load(std::sync::atomic::Ordering::Relaxed) {
        Ok(Some(
            PERF_MAP_RESOLVER.get_or_try_init(|| PerfMapResolver::new())?,
        ))
    } else {
        Ok(None)
    }
}

pub fn init_perfmap_resolver() {
    SHOULD_USE_PERF_MAP.store(true, std::sync::atomic::Ordering::Relaxed);
}
