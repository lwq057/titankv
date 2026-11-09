use crate::checksum::{crc32c_calculate, crc32c_verify};
use crate::error::TitanError;
use crate::index::record::IndexRecord;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_MAGIC: [u8; 4] = *b"TIDX";
pub const PAGE_VERSION: u16 = 1;
pub const HEADER_SIZE: usize = 18;
pub const CHECKSUM_SIZE: usize = 4;
pub const USABLE_PAGE_SIZE: usize = PAGE_SIZE - CHECKSUM_SIZE;

#[derive(Debug, Clone)]
pub struct BucketPage {
    pub flags: u16,
    pub next_overflow_page_id: u32,
    pub records: Vec<IndexRecord>,
}

impl BucketPage {
    pub fn new(flags: u16) -> Self {
        Self {
            flags,
            next_overflow_page_id: 0,
            records: Vec::new(),
        }
    }

    pub fn serialize(&self, buf: &mut [u8]) {
        assert_eq!(buf.len(), PAGE_SIZE);
        buf.fill(0);

        buf[0..4].copy_from_slice(&PAGE_MAGIC);
        buf[4..6].copy_from_slice(&PAGE_VERSION.to_le_bytes());
        buf[6..8].copy_from_slice(&self.flags.to_le_bytes());
        buf[8..10].copy_from_slice(&(self.records.len() as u16).to_le_bytes());

        let slot_dir_end = HEADER_SIZE + (self.records.len() * 4);
        let mut rec_data_offset = slot_dir_end;
        let mut slot_entries = Vec::with_capacity(self.records.len());
        let mut records_data = Vec::new();

        for rec in &self.records {
            let start = rec_data_offset;
            let before = records_data.len();
            rec.encode(&mut records_data);
            let len = records_data.len() - before;
            rec_data_offset += len;
            slot_entries.push((start as u16, len as u16));
        }

        buf[10..12].copy_from_slice(&(rec_data_offset as u16).to_le_bytes());
        buf[12..16].copy_from_slice(&self.next_overflow_page_id.to_le_bytes());
        buf[16..18].copy_from_slice(&0u16.to_le_bytes()); // Reserved

        // Write slots
        let mut slot_ptr = HEADER_SIZE;
        for (offset, len) in slot_entries {
            buf[slot_ptr..slot_ptr + 2].copy_from_slice(&offset.to_le_bytes());
            buf[slot_ptr + 2..slot_ptr + 4].copy_from_slice(&len.to_le_bytes());
            slot_ptr += 4;
        }

        // Write record bytes
        if !records_data.is_empty() && slot_ptr + records_data.len() <= USABLE_PAGE_SIZE {
            buf[slot_ptr..slot_ptr + records_data.len()].copy_from_slice(&records_data);
        }

        // Calculate checksum
        let checksum = crc32c_calculate(&buf[0..USABLE_PAGE_SIZE]);
        buf[USABLE_PAGE_SIZE..PAGE_SIZE].copy_from_slice(&checksum.to_le_bytes());
    }

    pub fn deserialize(buf: &[u8]) -> Result<Self, TitanError> {
        if buf.len() != PAGE_SIZE {
            return Err(TitanError::CorruptedRecord("Invalid page size".into()));
        }

        // Verify checksum
        let expected_checksum = u32::from_le_bytes(buf[USABLE_PAGE_SIZE..PAGE_SIZE].try_into().unwrap());
        if expected_checksum != 0 && !crc32c_verify(&buf[0..USABLE_PAGE_SIZE], expected_checksum) {
            return Err(TitanError::ChecksumError);
        }

        if &buf[0..4] != &PAGE_MAGIC {
            // Uninitialized page is empty
            if buf[0..4] == [0, 0, 0, 0] {
                return Ok(Self::new(0));
            }
            return Err(TitanError::CorruptedRecord("Invalid page magic".into()));
        }

        let version = u16::from_le_bytes(buf[4..6].try_into().unwrap());
        if version != PAGE_VERSION {
            return Err(TitanError::CorruptedRecord(format!(
                "Unsupported page version: {}",
                version
            )));
        }

        let flags = u16::from_le_bytes(buf[6..8].try_into().unwrap());
        let record_count = u16::from_le_bytes(buf[8..10].try_into().unwrap()) as usize;
        let next_overflow_page_id = u32::from_le_bytes(buf[12..16].try_into().unwrap());

        let mut records = Vec::with_capacity(record_count);
        let mut slot_offset = HEADER_SIZE;

        for _ in 0..record_count {
            if slot_offset + 4 > USABLE_PAGE_SIZE {
                break;
            }
            let rec_offset = u16::from_le_bytes(buf[slot_offset..slot_offset + 2].try_into().unwrap()) as usize;
            let rec_len = u16::from_le_bytes(buf[slot_offset + 2..slot_offset + 4].try_into().unwrap()) as usize;
            slot_offset += 4;

            if rec_offset + rec_len <= USABLE_PAGE_SIZE {
                if let Ok((rec, _)) = IndexRecord::decode(&buf[rec_offset..rec_offset + rec_len]) {
                    records.push(rec);
                }
            }
        }

        Ok(Self {
            flags,
            next_overflow_page_id,
            records,
        })
    }

    pub fn can_fit(&self, rec: &IndexRecord, replacing_existing_idx: Option<usize>) -> bool {
        let rec_size = rec.encoded_size();
        let slot_cost = if replacing_existing_idx.is_some() { 0 } else { 4 };
        let mut current_used = HEADER_SIZE + (self.records.len() * 4);
        for (i, r) in self.records.iter().enumerate() {
            if Some(i) != replacing_existing_idx {
                current_used += r.encoded_size();
            }
        }
        current_used + rec_size + slot_cost <= USABLE_PAGE_SIZE
    }

    pub fn find_record(&self, key: &[u8], hash: u64) -> Option<(usize, &IndexRecord)> {
        for (idx, r) in self.records.iter().enumerate() {
            if r.hash == hash && r.key == key {
                return Some((idx, r));
            }
        }
        None
    }

    pub fn upsert(&mut self, rec: IndexRecord) -> bool {
        if let Some((idx, _)) = self.find_record(&rec.key, rec.hash) {
            if self.can_fit(&rec, Some(idx)) {
                self.records[idx] = rec;
                return true;
            }
            return false;
        }

        if self.can_fit(&rec, None) {
            self.records.push(rec);
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, key: &[u8], hash: u64) -> bool {
        if let Some((idx, _)) = self.find_record(key, hash) {
            self.records.remove(idx);
            true
        } else {
            false
        }
    }
}
