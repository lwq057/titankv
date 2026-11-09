use crate::memory::budget::MemoryBudget;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

pub const PAGE_SIZE: usize = 4096;

pub struct BufferPool {
    budget: Arc<MemoryBudget>,
    pool_4k: Mutex<VecDeque<Vec<u8>>>,
    pool_16k: Mutex<VecDeque<Vec<u8>>>,
    pool_64k: Mutex<VecDeque<Vec<u8>>>,
    pool_128k: Mutex<VecDeque<Vec<u8>>>,
}

impl BufferPool {
    pub fn new(budget: Arc<MemoryBudget>, preheat_4k_count: usize) -> Self {
        let pool = Self {
            budget,
            pool_4k: Mutex::new(VecDeque::new()),
            pool_16k: Mutex::new(VecDeque::new()),
            pool_64k: Mutex::new(VecDeque::new()),
            pool_128k: Mutex::new(VecDeque::new()),
        };

        // Preheat 4KB page buffers within budget
        let mut preheated = VecDeque::new();
        for _ in 0..preheat_4k_count {
            if pool.budget.try_acquire_pool(PAGE_SIZE) {
                preheated.push_back(vec![0u8; PAGE_SIZE]);
            } else {
                break;
            }
        }
        *pool.pool_4k.lock() = preheated;

        pool
    }

    pub fn acquire_page(&self) -> PooledBuffer<'_> {
        let mut guard = self.pool_4k.lock();
        if let Some(mut buf) = guard.pop_front() {
            buf.fill(0);
            PooledBuffer {
                pool: self,
                capacity: PAGE_SIZE,
                buffer: buf,
            }
        } else {
            drop(guard);
            let has_budget = self.budget.try_acquire_pool(PAGE_SIZE);
            let buf = vec![0u8; PAGE_SIZE];
            PooledBuffer {
                pool: self,
                capacity: if has_budget { PAGE_SIZE } else { 0 },
                buffer: buf,
            }
        }
    }

    pub fn acquire_value_buffer(&self, min_size: usize) -> PooledBuffer<'_> {
        let target_size = if min_size <= 4096 {
            4096
        } else if min_size <= 16384 {
            16384
        } else if min_size <= 65536 {
            65536
        } else {
            131072
        };

        let pooled = match target_size {
            4096 => self.pool_4k.lock().pop_front(),
            16384 => self.pool_16k.lock().pop_front(),
            65536 => self.pool_64k.lock().pop_front(),
            _ => self.pool_128k.lock().pop_front(),
        };

        if let Some(mut buf) = pooled {
            buf.clear();
            PooledBuffer {
                pool: self,
                capacity: target_size,
                buffer: buf,
            }
        } else {
            let has_budget = self.budget.try_acquire_pool(target_size);
            let buf = Vec::with_capacity(target_size);
            PooledBuffer {
                pool: self,
                capacity: if has_budget { target_size } else { 0 },
                buffer: buf,
            }
        }
    }

    fn release(&self, capacity: usize, mut buf: Vec<u8>) {
        if capacity == 0 {
            return;
        }
        match capacity {
            4096 => {
                let mut guard = self.pool_4k.lock();
                if guard.len() < 512 {
                    buf.resize(4096, 0);
                    guard.push_back(buf);
                    return;
                }
            }
            16384 => {
                let mut guard = self.pool_16k.lock();
                if guard.len() < 128 {
                    buf.clear();
                    guard.push_back(buf);
                    return;
                }
            }
            65536 => {
                let mut guard = self.pool_64k.lock();
                if guard.len() < 64 {
                    buf.clear();
                    guard.push_back(buf);
                    return;
                }
            }
            131072 => {
                let mut guard = self.pool_128k.lock();
                if guard.len() < 32 {
                    buf.clear();
                    guard.push_back(buf);
                    return;
                }
            }
            _ => {}
        }
        self.budget.release_pool(capacity);
    }
}

pub struct PooledBuffer<'a> {
    pool: &'a BufferPool,
    capacity: usize,
    pub buffer: Vec<u8>,
}

impl<'a> std::ops::Deref for PooledBuffer<'a> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<'a> std::ops::DerefMut for PooledBuffer<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl<'a> Drop for PooledBuffer<'a> {
    fn drop(&mut self) {
        let capacity = self.capacity;
        let buf = std::mem::take(&mut self.buffer);
        self.pool.release(capacity, buf);
    }
}
