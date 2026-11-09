use crate::error::TitanError;
use crate::ttl::event::TtlEvent;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct TtlDiskManager {
    base_dir: PathBuf,
    ttl_bucket_width_ms: u64,
}

impl TtlDiskManager {
    pub fn new<P: AsRef<Path>>(
        data_dir: P,
        process_uuid: &str,
        lane_id: usize,
        ttl_bucket_width_ms: u64,
    ) -> Result<Self, TitanError> {
        let base_dir = data_dir
            .as_ref()
            .join("ttl")
            .join(process_uuid)
            .join(format!("lane-{}", lane_id));
        std::fs::create_dir_all(&base_dir)?;

        Ok(Self {
            base_dir,
            ttl_bucket_width_ms,
        })
    }

    pub fn append_event(&self, event: &TtlEvent) -> Result<(), TitanError> {
        if event.expire_at <= 0 {
            return Ok(());
        }

        let time_bucket = (event.expire_at as u64) / self.ttl_bucket_width_ms;
        let file_path = self.base_dir.join(format!("bucket-{}.ttl", time_bucket));

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        let mut buf = Vec::new();
        event.encode(&mut buf);
        file.write_all(&buf)?;
        Ok(())
    }
}
