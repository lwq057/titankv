use crate::error::TitanError;
use crate::io::{pread_exact, pwrite_all};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

pub struct Segment {
    pub segment_id: String,
    pub lane_id: usize,
    pub process_uuid: String,
    pub file_path: PathBuf,
    file: Arc<File>,
    current_offset: AtomicU64,
    reader_pins: AtomicU32,
    pub live_bytes: AtomicU64,
    pub total_bytes: AtomicU64,
}

impl Segment {
    pub fn segment_path<P: AsRef<Path>>(data_dir: P, segment_id: &str) -> PathBuf {
        let parts: Vec<&str> = segment_id.split('_').collect();
        if parts.len() >= 3 {
            let proc_uuid = parts[0];
            let lane_id = parts[1];
            data_dir
                .as_ref()
                .join("logs")
                .join(proc_uuid)
                .join(format!("lane-{}", lane_id))
                .join(format!("{}.log", segment_id))
        } else {
            data_dir
                .as_ref()
                .join("logs")
                .join(format!("{}.log", segment_id))
        }
    }

    pub fn open_by_id<P: AsRef<Path>>(data_dir: P, segment_id: &str) -> Result<Arc<Self>, TitanError> {
        let file_path = Self::segment_path(&data_dir, segment_id);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&file_path)?;

        let len = file.metadata()?.len();
        let parts: Vec<&str> = segment_id.split('_').collect();
        let (proc_uuid, lane_id) = if parts.len() >= 3 {
            (parts[0].to_string(), parts[1].parse::<usize>().unwrap_or(0))
        } else {
            ("unknown".to_string(), 0)
        };

        Ok(Arc::new(Self {
            segment_id: segment_id.to_string(),
            lane_id,
            process_uuid: proc_uuid,
            file_path,
            file: Arc::new(file),
            current_offset: AtomicU64::new(len),
            reader_pins: AtomicU32::new(0),
            live_bytes: AtomicU64::new(len),
            total_bytes: AtomicU64::new(len),
        }))
    }

    pub fn open_or_create<P: AsRef<Path>>(
        data_dir: P,
        _process_uuid: &str,
        _lane_id: usize,
        segment_id: &str,
    ) -> Result<Arc<Self>, TitanError> {
        Self::open_by_id(data_dir, segment_id)
    }

    pub fn append(&self, data: &[u8]) -> Result<u64, TitanError> {
        let offset = self.current_offset.fetch_add(data.len() as u64, Ordering::SeqCst);
        pwrite_all(&self.file, data, offset)?;
        self.total_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
        self.live_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(offset)
    }

    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), TitanError> {
        pread_exact(&self.file, buf, offset)?;
        Ok(())
    }

    pub fn sync(&self) -> Result<(), TitanError> {
        self.file.sync_data()?;
        Ok(())
    }

    pub fn current_size(&self) -> u64 {
        self.current_offset.load(Ordering::Relaxed)
    }

    pub fn pin(self: &Arc<Self>) -> SegmentPin {
        self.reader_pins.fetch_add(1, Ordering::SeqCst);
        SegmentPin {
            segment: Arc::clone(self),
        }
    }

    pub fn reader_count(&self) -> u32 {
        self.reader_pins.load(Ordering::SeqCst)
    }
}

pub struct SegmentPin {
    segment: Arc<Segment>,
}

impl Drop for SegmentPin {
    fn drop(&mut self) {
        self.segment.reader_pins.fetch_sub(1, Ordering::SeqCst);
    }
}
