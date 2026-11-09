use crate::ttl::event::TtlEvent;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HeapItem {
    pub expire_at: i64,
    pub shard_id: u16,
    pub local_bucket_id: u32,
    pub sequence: u64,
    pub key: Vec<u8>,
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for Min-Heap
        other.expire_at.cmp(&self.expire_at)
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct BoundedTtlHeap {
    capacity: usize,
    heap: BinaryHeap<HeapItem>,
}

impl BoundedTtlHeap {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            heap: BinaryHeap::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, event: TtlEvent) -> bool {
        if self.heap.len() >= self.capacity {
            return false;
        }
        self.heap.push(HeapItem {
            expire_at: event.expire_at,
            shard_id: event.shard_id,
            local_bucket_id: event.local_bucket_id,
            sequence: event.sequence,
            key: event.key,
        });
        true
    }

    pub fn peek_expired(&self, now_ms: i64) -> Option<&HeapItem> {
        if let Some(top) = self.heap.peek() {
            if top.expire_at <= now_ms {
                return Some(top);
            }
        }
        None
    }

    pub fn pop(&mut self) -> Option<HeapItem> {
        self.heap.pop()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}
