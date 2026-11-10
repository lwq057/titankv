use crate::error::TitanError;
use crate::io::{pread_exact, pwrite_all};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub struct OverflowAllocator {
    pub shard_id: usize,
    pub buckets_per_shard: usize,
    pub file_path: PathBuf,
    file: Arc<File>,
    next_page_id: AtomicU32,
}

impl OverflowAllocator {
    pub fn new<P: AsRef<Path>>(
        data_dir: P,
        shard_id: usize,
        buckets_per_shard: usize,
    ) -> Result<Self, TitanError> {
        let index_dir = data_dir.as_ref().join("index");
        std::fs::create_dir_all(&index_dir)?;
        let file_path = index_dir.join(format!("shard-{}.idx", shard_id));

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&file_path)?;

        let meta = file.metadata()?;
        let file_len = meta.len();
        let total_pages = (file_len / 4096) as u32;
        let start_page = (buckets_per_shard as u32).max(total_pages);

        Ok(Self {
            shard_id,
            buckets_per_shard,
            file_path,
            file: Arc::new(file),
            next_page_id: AtomicU32::new(start_page),
        })
    }

    pub fn file(&self) -> Arc<File> {
        Arc::clone(&self.file)
    }

    pub fn allocate_overflow_page(&self) -> u32 {
        if let Ok(meta) = self.file.metadata() {
            let disk_pages = (meta.len() / 4096) as u32;
            let min_page = (self.buckets_per_shard as u32).max(disk_pages);
            self.next_page_id.fetch_max(min_page, Ordering::SeqCst);
        }
        self.next_page_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn read_page(&self, page_id: u32, buf: &mut [u8]) -> Result<(), TitanError> {
        let offset = page_id as u64 * 4096;
        let file_len = self.file.metadata()?.len();
        if offset >= file_len {
            buf.fill(0);
            return Ok(());
        }
        pread_exact(&self.file, buf, offset)?;
        Ok(())
    }

    pub fn write_page(&self, page_id: u32, buf: &[u8]) -> Result<(), TitanError> {
        let offset = page_id as u64 * 4096;
        pwrite_all(&self.file, buf, offset)?;
        Ok(())
    }
}
