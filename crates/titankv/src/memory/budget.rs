use crate::config::MemoryConfig;
use crate::error::TitanError;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct MemoryBudget {
    pub max_bytes: usize,
    pub queue_limit: usize,
    pub inflight_limit: usize,
    pub buffer_pool_limit: usize,

    current_queue_bytes: AtomicUsize,
    current_inflight_bytes: AtomicUsize,
    current_buffer_pool_bytes: AtomicUsize,
}

impl MemoryBudget {
    pub fn new(config: &MemoryConfig) -> Self {
        Self {
            max_bytes: config.max_bytes,
            queue_limit: config.queue_bytes,
            inflight_limit: config.in_flight_bytes,
            buffer_pool_limit: config.buffer_pool_bytes,
            current_queue_bytes: AtomicUsize::new(0),
            current_inflight_bytes: AtomicUsize::new(0),
            current_buffer_pool_bytes: AtomicUsize::new(0),
        }
    }

    pub fn total_used(&self) -> usize {
        self.current_queue_bytes.load(Ordering::Relaxed)
            + self.current_inflight_bytes.load(Ordering::Relaxed)
            + self.current_buffer_pool_bytes.load(Ordering::Relaxed)
    }

    pub fn acquire_queue(&self, bytes: usize) -> Result<QueueGuard<'_>, TitanError> {
        let cur = self.current_queue_bytes.load(Ordering::Relaxed);
        if cur + bytes > self.queue_limit || self.total_used() + bytes > self.max_bytes {
            return Err(TitanError::QueueFull);
        }
        self.current_queue_bytes.fetch_add(bytes, Ordering::Relaxed);
        Ok(QueueGuard {
            budget: self,
            bytes,
        })
    }

    pub fn acquire_inflight(&self, bytes: usize) -> Result<InflightGuard<'_>, TitanError> {
        let cur = self.current_inflight_bytes.load(Ordering::Relaxed);
        if cur + bytes > self.inflight_limit || self.total_used() + bytes > self.max_bytes {
            return Err(TitanError::MemoryLimit);
        }
        self.current_inflight_bytes.fetch_add(bytes, Ordering::Relaxed);
        Ok(InflightGuard {
            budget: self,
            bytes,
        })
    }

    pub fn try_acquire_pool(&self, bytes: usize) -> bool {
        let cur = self.current_buffer_pool_bytes.load(Ordering::Relaxed);
        if cur + bytes > self.buffer_pool_limit || self.total_used() + bytes > self.max_bytes {
            false
        } else {
            self.current_buffer_pool_bytes.fetch_add(bytes, Ordering::Relaxed);
            true
        }
    }

    pub fn release_pool(&self, bytes: usize) {
        self.current_buffer_pool_bytes.fetch_sub(bytes, Ordering::Relaxed);
    }
}

pub struct QueueGuard<'a> {
    budget: &'a MemoryBudget,
    bytes: usize,
}

impl<'a> Drop for QueueGuard<'a> {
    fn drop(&mut self) {
        self.budget.current_queue_bytes.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}

pub struct InflightGuard<'a> {
    budget: &'a MemoryBudget,
    bytes: usize,
}

impl<'a> Drop for InflightGuard<'a> {
    fn drop(&mut self) {
        self.budget.current_inflight_bytes.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
