use crate::error::TitanError;
use crate::index::allocator::OverflowAllocator;
use crate::index::page::{BucketPage, PAGE_SIZE};
use crate::index::record::IndexRecord;
use std::collections::HashSet;
use std::sync::Arc;

pub struct BucketManager {
    allocators: Vec<Arc<OverflowAllocator>>,
    #[allow(dead_code)]
    buckets_per_shard: usize,
}

impl BucketManager {
    pub fn new(allocators: Vec<Arc<OverflowAllocator>>, buckets_per_shard: usize) -> Self {
        Self {
            allocators,
            buckets_per_shard,
        }
    }

    pub fn get_record(
        &self,
        shard_id: usize,
        local_bucket_id: usize,
        key: &[u8],
        hash: u64,
    ) -> Result<Option<IndexRecord>, TitanError> {
        let alloc = &self.allocators[shard_id];
        let mut current_page_id = Some(local_bucket_id as u32);
        let mut visited = HashSet::new();
        let mut buf = vec![0u8; PAGE_SIZE];

        while let Some(pid) = current_page_id {
            if !visited.insert(pid) {
                return Err(TitanError::CorruptedRecord(
                    "Overflow page cycle detected in index".into(),
                ));
            }

            alloc.read_page(pid, &mut buf)?;
            let page = BucketPage::deserialize(&buf)?;

            if let Some((_, rec)) = page.find_record(key, hash) {
                return Ok(Some(rec.clone()));
            }

            if page.next_overflow_page_id != 0 {
                current_page_id = Some(page.next_overflow_page_id);
            } else {
                current_page_id = None;
            }
        }

        Ok(None)
    }

    pub fn upsert_record(
        &self,
        shard_id: usize,
        local_bucket_id: usize,
        record: IndexRecord,
    ) -> Result<(), TitanError> {
        let alloc = &self.allocators[shard_id];
        let primary_page_id = local_bucket_id as u32;
        let mut buf = vec![0u8; PAGE_SIZE];

        // 1. First pass: see if record already exists in any page of the bucket chain
        let mut current_page_id = Some(primary_page_id);
        let mut visited = HashSet::new();

        while let Some(pid) = current_page_id {
            if !visited.insert(pid) {
                return Err(TitanError::CorruptedRecord("Overflow cycle detected".into()));
            }

            alloc.read_page(pid, &mut buf)?;
            let mut page = BucketPage::deserialize(&buf)?;

            if page.find_record(&record.key, record.hash).is_some() {
                // Try updating in place
                if page.upsert(record.clone()) {
                    page.serialize(&mut buf);
                    alloc.write_page(pid, &buf)?;
                    return Ok(());
                } else {
                    // Remove from this page and reinsert elsewhere
                    page.remove(&record.key, record.hash);
                    page.serialize(&mut buf);
                    alloc.write_page(pid, &buf)?;
                    break;
                }
            }

            if page.next_overflow_page_id != 0 {
                current_page_id = Some(page.next_overflow_page_id);
            } else {
                current_page_id = None;
            }
        }

        // 2. Insert into the first page with enough space, or allocate an overflow page
        current_page_id = Some(primary_page_id);
        let mut last_page_id = primary_page_id;
        visited.clear();

        while let Some(pid) = current_page_id {
            visited.insert(pid);
            last_page_id = pid;

            alloc.read_page(pid, &mut buf)?;
            let mut page = BucketPage::deserialize(&buf)?;

            if page.upsert(record.clone()) {
                page.serialize(&mut buf);
                alloc.write_page(pid, &buf)?;
                return Ok(());
            }

            if page.next_overflow_page_id != 0 {
                current_page_id = Some(page.next_overflow_page_id);
            } else {
                current_page_id = None;
            }
        }

        // Allocate a new overflow page and link to last page
        let new_overflow_page_id = alloc.allocate_overflow_page();
        let mut new_page = BucketPage::new(2);
        new_page.upsert(record);
        new_page.serialize(&mut buf);
        alloc.write_page(new_overflow_page_id, &buf)?;

        // Update last page's next pointer
        alloc.read_page(last_page_id, &mut buf)?;
        let mut last_page = BucketPage::deserialize(&buf)?;
        last_page.next_overflow_page_id = new_overflow_page_id;
        last_page.serialize(&mut buf);
        alloc.write_page(last_page_id, &buf)?;

        Ok(())
    }

    pub fn delete_record(
        &self,
        shard_id: usize,
        local_bucket_id: usize,
        key: &[u8],
        hash: u64,
    ) -> Result<bool, TitanError> {
        let alloc = &self.allocators[shard_id];
        let mut current_page_id = Some(local_bucket_id as u32);
        let mut visited = HashSet::new();
        let mut buf = vec![0u8; PAGE_SIZE];

        while let Some(pid) = current_page_id {
            if !visited.insert(pid) {
                return Err(TitanError::CorruptedRecord("Overflow cycle detected".into()));
            }

            alloc.read_page(pid, &mut buf)?;
            let mut page = BucketPage::deserialize(&buf)?;

            if page.remove(key, hash) {
                page.serialize(&mut buf);
                alloc.write_page(pid, &buf)?;
                return Ok(true);
            }

            if page.next_overflow_page_id != 0 {
                current_page_id = Some(page.next_overflow_page_id);
            } else {
                current_page_id = None;
            }
        }

        Ok(false)
    }
}
