use crate::error::TitanError;
use crate::log::segment::Segment;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct LaneWriter {
    data_dir: PathBuf,
    process_uuid: String,
    lane_id: usize,
    max_segment_size: u64,
    current_segment: Mutex<Arc<Segment>>,
    segment_seq: AtomicU64,
    open_segments: Arc<Mutex<HashMap<String, Arc<Segment>>>>,
}

impl LaneWriter {
    pub fn new<P: AsRef<Path>>(
        data_dir: P,
        process_uuid: &str,
        lane_id: usize,
        max_segment_size: u64,
        open_segments: Arc<Mutex<HashMap<String, Arc<Segment>>>>,
    ) -> Result<Self, TitanError> {
        let seg_id = format!("{}_{}_{}", process_uuid, lane_id, 1);
        let segment = Segment::open_or_create(&data_dir, process_uuid, lane_id, &seg_id)?;

        open_segments.lock().insert(seg_id.clone(), Arc::clone(&segment));

        Ok(Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            process_uuid: process_uuid.to_string(),
            lane_id,
            max_segment_size,
            current_segment: Mutex::new(segment),
            segment_seq: AtomicU64::new(1),
            open_segments,
        })
    }

    pub fn append(&self, record_bytes: &[u8]) -> Result<(String, u64), TitanError> {
        let mut cur_guard = self.current_segment.lock();

        // Check if current segment needs rolling
        if cur_guard.current_size() + record_bytes.len() as u64 > self.max_segment_size {
            let next_seq = self.segment_seq.fetch_add(1, Ordering::SeqCst) + 1;
            let new_seg_id = format!("{}_{}_{}", self.process_uuid, self.lane_id, next_seq);
            let new_segment = Segment::open_or_create(
                &self.data_dir,
                &self.process_uuid,
                self.lane_id,
                &new_seg_id,
            )?;
            self.open_segments
                .lock()
                .insert(new_seg_id.clone(), Arc::clone(&new_segment));
            *cur_guard = new_segment;
        }

        let seg_id = cur_guard.segment_id.clone();
        let offset = cur_guard.append(record_bytes)?;
        Ok((seg_id, offset))
    }

    pub fn sync_current(&self) -> Result<(), TitanError> {
        self.current_segment.lock().sync()
    }
}
