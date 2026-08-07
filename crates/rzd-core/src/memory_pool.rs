use crate::events::SimEvent;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

thread_local! {
    /// Thread-local event pool for zero-allocation event reuse
    static EVENT_POOL: std::cell::RefCell<VecDeque<SimEvent>> = const { std::cell::RefCell::new(VecDeque::new()) };
}

/// Global memory pool metrics
pub struct MemoryPoolMetrics {
    pub allocated: AtomicU64,
    pub reused: AtomicU64,
    pub pool_capacity: AtomicU64,
    pub total_bytes: AtomicU64,
    pub peak_pool_size: AtomicU64,
}

impl MemoryPoolMetrics {
    pub fn new() -> Self {
        MemoryPoolMetrics {
            allocated: AtomicU64::new(0),
            reused: AtomicU64::new(0),
            pool_capacity: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            peak_pool_size: AtomicU64::new(0),
        }
    }

    pub fn reuse_rate(&self) -> f64 {
        let total = self.allocated.load(Ordering::Relaxed) + self.reused.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            self.reused.load(Ordering::Relaxed) as f64 / total as f64
        }
    }
}

impl Default for MemoryPoolMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory pool for events with reuse strategy
pub struct EventPool {
    pool: Vec<SimEvent>,
    max_pool_size: usize,
    metrics: Arc<MemoryPoolMetrics>,
    reuse_count: u64,
    miss_count: u64,
}

impl EventPool {
    pub fn new(max_pool_size: usize) -> Self {
        let capacity = max_pool_size.min(4096);
        EventPool {
            pool: Vec::with_capacity(capacity),
            max_pool_size,
            metrics: Arc::new(MemoryPoolMetrics::new()),
            reuse_count: 0,
            miss_count: 0,
        }
    }

    /// Acquire an event from the pool
    pub fn acquire(&mut self) -> SimEvent {
        if let Some(mut event) = self.pool.pop() {
            self.reuse_count += 1;
            self.metrics.reused.fetch_add(1, Ordering::Relaxed);
            event.reset();
            event
        } else {
            self.miss_count += 1;
            self.metrics.allocated.fetch_add(1, Ordering::Relaxed);
            SimEvent::new()
        }
    }

    /// Return an event to the pool for reuse
    pub fn release(&mut self, event: SimEvent) {
        if self.pool.len() < self.max_pool_size {
            self.pool.push(event);
            let len = self.pool.len() as u64;
            self.metrics.pool_capacity.store(len, Ordering::Relaxed);
            if len > self.metrics.peak_pool_size.load(Ordering::Relaxed) {
                self.metrics.peak_pool_size.store(len, Ordering::Relaxed);
            }
        }
    }

    /// Current pool utilization
    pub fn utilization(&self) -> (u64, u64) {
        (self.reuse_count, self.miss_count)
    }

    /// Reuse rate as a percentage (0.0 - 1.0)
    pub fn reuse_rate(&self) -> f64 {
        let total = self.reuse_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.reuse_count as f64 / total as f64
        }
    }
}

/// Performance tracking for event processing throughput
pub struct ThroughputTracker {
    event_count: AtomicU64,
    start: Instant,
    window_events: parking_lot::Mutex<VecDeque<(Instant, u64)>>,
}

impl ThroughputTracker {
    pub fn new() -> Self {
        ThroughputTracker {
            event_count: AtomicU64::new(0),
            start: Instant::now(),
            window_events: parking_lot::Mutex::new(VecDeque::with_capacity(1000)),
        }
    }

    pub fn record_event(&self) {
        let prev = self.event_count.fetch_add(1, Ordering::Relaxed);
        let mut window = self.window_events.lock();
        if window.len() >= 1000 {
            window.pop_front();
        }
        window.push_back((Instant::now(), prev + 1));
    }

    pub fn total_events(&self) -> u64 {
        self.event_count.load(Ordering::Relaxed)
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn events_per_second(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        if elapsed == 0.0 {
            0.0
        } else {
            self.event_count.load(Ordering::Relaxed) as f64 / elapsed
        }
    }

    pub fn recent_eps(&self) -> f64 {
        let window = self.window_events.lock();
        let count = window.len();
        if count < 2 {
            return 0.0;
        }
        let first = window.front().unwrap();
        let last = window.back().unwrap();
        let elapsed = last.0.duration_since(first.0).as_secs_f64();
        if elapsed == 0.0 {
            0.0
        } else {
            (last.1 - first.1) as f64 / elapsed
        }
    }
}

impl Default for ThroughputTracker {
    fn default() -> Self {
        Self::new()
    }
}
