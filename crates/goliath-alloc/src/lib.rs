//! Owns diagnostic Rust allocation counts and bounded sampled live-stack capture.
//!
//! The application explicitly selects this allocator and starts capture. No
//! production model/store dependency or library-owned thread uses this crate.

use std::alloc::{GlobalAlloc, Layout, System};
use std::backtrace::Backtrace;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

pub struct Allocator;

#[repr(align(128))]
struct Shard {
    allocations: AtomicU64,
    deallocations: AtomicU64,
    allocated_bytes: AtomicU64,
    deallocated_bytes: AtomicU64,
    live_bytes: AtomicI64,
    live_blocks: AtomicI64,
    selector: AtomicU64,
}
impl Shard {
    const fn new() -> Self {
        Self {
            allocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            allocated_bytes: AtomicU64::new(0),
            deallocated_bytes: AtomicU64::new(0),
            live_bytes: AtomicI64::new(0),
            live_blocks: AtomicI64::new(0),
            selector: AtomicU64::new(0),
        }
    }
}
const SHARDS: usize = 256;
static COUNTERS: [Shard; SHARDS] = [const { Shard::new() }; SHARDS];
static NEXT_SHARD: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_PEAK: AtomicUsize = AtomicUsize::new(0);
thread_local! {static SHARD: Cell<usize> = const {Cell::new(usize::MAX)};}

fn current() -> &'static Shard {
    let index = SHARD
        .try_with(|slot| {
            let mut index = slot.get();
            if index == usize::MAX {
                index = NEXT_SHARD.fetch_add(1, Ordering::Relaxed) % SHARDS;
                slot.set(index);
            }
            index
        })
        .unwrap_or(0);
    &COUNTERS[index]
}
static ENABLED: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static MISSED_TLS: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static EVERY: AtomicU64 = AtomicU64::new(2048);
static MAX_SAMPLES: AtomicUsize = AtomicUsize::new(4096);
const BUCKETS: usize = 8192;
static OCCUPIED: [AtomicU32; BUCKETS] = [const { AtomicU32::new(0) }; BUCKETS];
static SAMPLES: Mutex<Option<HashMap<usize, Sample>>> = Mutex::new(None);
thread_local! { static BUSY: Cell<bool> = const { Cell::new(false) }; }

struct Sample {
    bytes: usize,
    born: Instant,
    period: u64,
    stack: Backtrace,
}

#[derive(Clone, Copy, Debug)]
pub struct Counts {
    pub allocations: u64,
    pub deallocations: u64,
    pub allocated_bytes: u64,
    pub deallocated_bytes: u64,
    pub live_bytes: usize,
    pub live_blocks: usize,
    /// Highest byte count observed by snapshots, not an instantaneous peak.
    pub peak_bytes: usize,
}

pub struct Retained {
    pub bytes: usize,
    pub age: Duration,
    pub period: u64,
    pub stack: String,
}

pub struct Capture {
    pub counts: Counts,
    pub dropped_samples: u64,
    pub missed_tls_callbacks: u64,
    pub sampler_failed: bool,
    pub samples: Vec<Retained>,
}

pub fn counts() -> Counts {
    let mut counts = Counts {
        allocations: 0,
        deallocations: 0,
        allocated_bytes: 0,
        deallocated_bytes: 0,
        live_bytes: 0,
        live_blocks: 0,
        peak_bytes: 0,
    };
    let (mut live_bytes, mut live_blocks) = (0i128, 0i128);
    for shard in &COUNTERS {
        counts.allocations = counts
            .allocations
            .saturating_add(shard.allocations.load(Ordering::Relaxed));
        counts.deallocations = counts
            .deallocations
            .saturating_add(shard.deallocations.load(Ordering::Relaxed));
        counts.allocated_bytes = counts
            .allocated_bytes
            .saturating_add(shard.allocated_bytes.load(Ordering::Relaxed));
        counts.deallocated_bytes = counts
            .deallocated_bytes
            .saturating_add(shard.deallocated_bytes.load(Ordering::Relaxed));
        live_bytes += i128::from(shard.live_bytes.load(Ordering::Relaxed));
        live_blocks += i128::from(shard.live_blocks.load(Ordering::Relaxed));
    }
    // Cross-thread frees can make individual shards negative. A concurrent
    // aggregate spans several instants; quiescent aggregates are exact.
    counts.live_bytes = live_bytes.clamp(0, usize::MAX as i128) as usize;
    counts.live_blocks = live_blocks.clamp(0, usize::MAX as i128) as usize;
    OBSERVED_PEAK.fetch_max(counts.live_bytes, Ordering::Relaxed);
    counts.peak_bytes = OBSERVED_PEAK.load(Ordering::Relaxed);
    counts
}

fn bucket(address: usize) -> usize {
    (address
        .wrapping_mul(0x9e37_79b9_7f4a_7c15usize)
        .rotate_right(17))
        & (BUCKETS - 1)
}

/// Diagnostic failure disables only sampling. Callbacks never alter application
/// pointers or prevent the System operation. Never unwind through GlobalAlloc.
fn guarded<T>(work: impl FnOnce() -> T) -> Option<T> {
    let entered = BUSY.try_with(|busy| {
        if busy.get() {
            false
        } else {
            busy.set(true);
            true
        }
    });
    match entered {
        Ok(false) => return None,
        Err(_) => {
            MISSED_TLS.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        Ok(true) => {}
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
    let _ = BUSY.try_with(|busy| busy.set(false));
    match result {
        Ok(value) => Some(value),
        Err(_) => {
            FAILED.store(true, Ordering::Relaxed);
            ENABLED.store(false, Ordering::Release);
            None
        }
    }
}

fn allocated(address: usize, bytes: usize) {
    let counters = current();
    counters.allocations.fetch_add(1, Ordering::Relaxed);
    counters
        .allocated_bytes
        .fetch_add(bytes as u64, Ordering::Relaxed);
    counters.live_blocks.fetch_add(1, Ordering::Relaxed);
    counters
        .live_bytes
        .fetch_add(bytes as i64, Ordering::Relaxed);
    if !ENABLED.load(Ordering::Acquire) || BUSY.try_with(Cell::get).unwrap_or(true) {
        return;
    }
    let period = if bytes >= 1024 * 1024 {
        1
    } else if bytes >= 64 * 1024 {
        32
    } else {
        EVERY.load(Ordering::Relaxed)
    };
    let sequence = counters.selector.fetch_add(1, Ordering::Relaxed);
    if !sequence.is_multiple_of(period) {
        return;
    }
    guarded(|| {
        let stack = Backtrace::force_capture();
        let Ok(mut map) = SAMPLES.lock() else {
            FAILED.store(true, Ordering::Relaxed);
            return;
        };
        let Some(map) = map.as_mut() else {
            return;
        };
        if map.len() >= MAX_SAMPLES.load(Ordering::Relaxed) {
            DROPPED.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let old = map.insert(
            address,
            Sample {
                bytes,
                born: Instant::now(),
                period,
                stack,
            },
        );
        if old.is_none() {
            OCCUPIED[bucket(address)].fetch_add(1, Ordering::Release);
        }
    });
}

fn deallocated(address: usize, bytes: usize) {
    released(bytes);
    drop(remove_sample(address));
}

fn released(bytes: usize) {
    let counters = current();
    counters.deallocations.fetch_add(1, Ordering::Relaxed);
    counters
        .deallocated_bytes
        .fetch_add(bytes as u64, Ordering::Relaxed);
    counters.live_blocks.fetch_sub(1, Ordering::Relaxed);
    counters
        .live_bytes
        .fetch_sub(bytes as i64, Ordering::Relaxed);
}

fn remove_sample(address: usize) -> Option<Sample> {
    if !ENABLED.load(Ordering::Acquire) || OCCUPIED[bucket(address)].load(Ordering::Acquire) == 0 {
        return None;
    }
    guarded(|| {
        let Ok(mut map) = SAMPLES.lock() else {
            FAILED.store(true, Ordering::Relaxed);
            return None;
        };
        let removed = map.as_mut().and_then(|map| map.remove(&address));
        if removed.is_some() {
            OCCUPIED[bucket(address)].fetch_sub(1, Ordering::Release);
        }
        removed
    })
    .flatten()
}

fn restore_sample(address: usize, sample: Option<Sample>) {
    if let Some(sample) = sample {
        guarded(|| {
            let Ok(mut map) = SAMPLES.lock() else {
                FAILED.store(true, Ordering::Relaxed);
                return;
            };
            if let Some(map) = map.as_mut() {
                map.insert(address, sample);
                OCCUPIED[bucket(address)].fetch_add(1, Ordering::Release);
            } else {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        });
    }
}

// SAFETY: Every operation forwards its original pointer/layout to System.
// Counts and guarded sampling never alter the allocation or unwind to callers.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc's caller supplies a valid nonzero layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(pointer as usize, layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The same valid layout and zero-initialization contract are forwarded.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(pointer as usize, layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // Retire the address before System can make it available to another thread.
        deallocated(pointer as usize, layout.size());
        // SAFETY: The pointer belongs to this forwarding allocator with this layout.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let sample = remove_sample(pointer as usize);
        // SAFETY: GlobalAlloc's caller guarantees the original allocation and new size.
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            released(layout.size());
            allocated(next as usize, size);
        } else {
            restore_sample(pointer as usize, sample);
        }
        next
    }
}

pub fn start_capture(every: u64, max_samples: usize) -> Result<(), &'static str> {
    if every == 0 || max_samples == 0 || max_samples > 16384 {
        return Err("invalid bounded sampling configuration");
    }
    if STARTED.swap(true, Ordering::AcqRel) {
        return Err("capture is one-shot");
    }
    guarded(|| {
        let mut samples = SAMPLES.lock().map_err(|_| "sample map poisoned")?;
        *samples = Some(HashMap::new());
        EVERY.store(every, Ordering::Relaxed);
        MAX_SAMPLES.store(max_samples, Ordering::Relaxed);
        ENABLED.store(true, Ordering::Release);
        Ok(())
    })
    .ok_or("sampler initialization failed")?
}

pub fn stop_capture() -> Result<Capture, &'static str> {
    if !STARTED.load(Ordering::Acquire) {
        return Err("capture has not started");
    }
    ENABLED.store(false, Ordering::Release);
    guarded(|| {
        let mut map = SAMPLES.lock().map_err(|_| "sample map poisoned")?;
        let samples = map.take().ok_or("capture was already dumped")?;
        let now = Instant::now();
        let counts = counts();
        let samples = samples
            .into_values()
            .map(|sample| Retained {
                bytes: sample.bytes,
                age: now.saturating_duration_since(sample.born),
                period: sample.period,
                stack: format!("{}", sample.stack),
            })
            .collect();
        Ok(Capture {
            counts,
            samples,
            dropped_samples: DROPPED.load(Ordering::Relaxed),
            missed_tls_callbacks: MISSED_TLS.load(Ordering::Relaxed),
            sampler_failed: FAILED.load(Ordering::Relaxed),
        })
    })
    .ok_or("sampler snapshot failed")?
}

#[cfg(test)]
mod tests {
    //! Owns forwarding layouts, cross-thread release and bounded sample retention.
    use super::*;

    #[test]
    fn forwarding_and_capture_preserve_memory_and_remove_freed_samples() {
        let before = counts();
        start_capture(1, 64).unwrap();
        let layout = Layout::from_size_align(64, 16).unwrap();
        // SAFETY: These pointers are checked, used only within their layouts,
        // reallocated through the same allocator and released exactly once.
        let pointer = unsafe { Allocator.alloc_zeroed(layout) };
        assert!(!pointer.is_null());
        // SAFETY: The checked allocation contains 64 initialized bytes.
        let initialized = unsafe { std::slice::from_raw_parts(pointer, 64) };
        assert!(initialized.iter().all(|b| *b == 0));
        // SAFETY: The original pointer/layout are valid and the new size is nonzero.
        let pointer = unsafe { Allocator.realloc(pointer, layout, 128) };
        assert!(!pointer.is_null());
        let address = pointer as usize;
        std::thread::spawn(move || {
            // SAFETY: Ownership of this allocation was transferred by its address;
            // no other thread uses it and the new layout preserves alignment.
            unsafe {
                Allocator.dealloc(
                    address as *mut u8,
                    Layout::from_size_align(128, 16).unwrap(),
                )
            };
        })
        .join()
        .unwrap();
        let retained = Layout::from_size_align(32, 8).unwrap();
        // SAFETY: This nonzero layout is valid; the resulting allocation is freed below.
        let pointer = unsafe { Allocator.alloc(retained) };
        assert!(!pointer.is_null());
        let capture = stop_capture().unwrap();
        assert!(!capture.sampler_failed);
        assert_eq!(capture.missed_tls_callbacks, 0);
        assert_eq!(capture.samples.len(), 1);
        assert_eq!(capture.samples[0].bytes, 32);
        assert_eq!(capture.counts.live_bytes, before.live_bytes + 32);
        // SAFETY: This is the sole release of the retained pointer with its layout.
        unsafe { Allocator.dealloc(pointer, retained) };
        assert_eq!(counts().live_bytes, before.live_bytes);
        assert!(start_capture(1, 64).is_err());
    }
}
