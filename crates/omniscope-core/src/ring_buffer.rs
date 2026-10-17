use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// A thread-safe circular ring buffer with atomic overflow tracking and selective query capabilities.
#[derive(Debug)]
pub struct RingBuffer<T> {
    capacity: usize,
    buffer: RwLock<VecDeque<T>>,
    total_pushed: AtomicU64,
    dropped_count: AtomicU64,
}

impl<T> RingBuffer<T> {
    /// Creates a new `RingBuffer` with the specified maximum capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be greater than zero");
        Self {
            capacity,
            buffer: RwLock::new(VecDeque::with_capacity(capacity)),
            total_pushed: AtomicU64::new(0),
            dropped_count: AtomicU64::new(0),
        }
    }

    /// Pushes an item into the buffer. If the buffer is at maximum capacity, the oldest item is evicted
    /// and the dropped item counter is atomically incremented.
    pub fn push_overwrite(&self, item: T) {
        let mut guard = self.buffer.write().expect("RingBuffer lock poisoned");
        if guard.len() >= self.capacity {
            guard.pop_front();
            self.dropped_count.fetch_add(1, Ordering::Relaxed);
        }
        guard.push_back(item);
        self.total_pushed.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns the total number of items pushed into the buffer across its lifetime.
    pub fn total_pushed(&self) -> u64 {
        self.total_pushed.load(Ordering::Relaxed)
    }

    /// Returns the number of items that have been dropped due to capacity overflow.
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    /// Returns the current number of items stored in the buffer.
    pub fn len(&self) -> usize {
        let guard = self.buffer.read().expect("RingBuffer lock poisoned");
        guard.len()
    }

    /// Returns whether the buffer currently contains zero items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the configured capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all items currently in the buffer.
    pub fn clear(&self) {
        let mut guard = self.buffer.write().expect("RingBuffer lock poisoned");
        guard.clear();
    }

    /// Returns up to `limit` of the most recent items in chronological order.
    pub fn get_latest(&self, limit: usize) -> Vec<T>
    where
        T: Clone,
    {
        let guard = self.buffer.read().expect("RingBuffer lock poisoned");
        let count = limit.min(guard.len());
        let start_idx = guard.len() - count;
        guard.iter().skip(start_idx).cloned().collect()
    }

    /// Evaluates `filter` on items from newest to oldest, returning up to `limit` matching items
    /// sorted in chronological order.
    pub fn get_slice<F>(&self, filter: F, limit: usize) -> Vec<T>
    where
        T: Clone,
        F: Fn(&T) -> bool,
    {
        let guard = self.buffer.read().expect("RingBuffer lock poisoned");
        let mut matched = Vec::with_capacity(limit.min(guard.len()));

        // Scan backwards from newest to oldest
        for item in guard.iter().rev() {
            if filter(item) {
                matched.push(item.clone());
                if matched.len() >= limit {
                    break;
                }
            }
        }

        // Reverse back to restore chronological order (oldest to newest)
        matched.reverse();
        matched
    }
}
