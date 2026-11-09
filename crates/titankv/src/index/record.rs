use crate::error::TitanError;

pub const FLAG_TOMBSTONE: u16 = 0x0001;
pub const FLAG_COMPRESSED: u16 = 0x0002;
pub const FLAG_LZ4: u16 = 0x0004;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRecord {
    pub hash: u64,
    pub key: Vec<u8>,
    pub segment_id: String,
    pub value_offset: u64,
    pub stored_len: u32,
    pub raw_len: u32,
    pub expire_at: i64,
    pub sequence: u64,
    pub relocation_generation: u32,
    pub flags: u16,
}

impl IndexRecord {
    pub fn is_tombstone(&self) -> bool {
        (self.flags & FLAG_TOMBSTONE) != 0
    }

    pub fn is_expired(&self, now_ms: i64) -> bool {
        self.expire_at > 0 && now_ms >= self.expire_at
    }

    pub fn encoded_size(&self) -> usize {
        // hash (8) + key_len (1) + key + seg_id_len (1) + seg_id + offset (8)
        // + stored_len (4) + raw_len (4) + expire_at (8) + seq (8) + reloc_gen (4) + flags (2)
        8 + 1 + self.key.len() + 1 + self.segment_id.len() + 8 + 4 + 4 + 8 + 8 + 4 + 2
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.hash.to_le_bytes());
        buf.push(self.key.len() as u8);
        buf.extend_from_slice(&self.key);
        let seg_bytes = self.segment_id.as_bytes();
        buf.push(seg_bytes.len() as u8);
        buf.extend_from_slice(seg_bytes);
        buf.extend_from_slice(&self.value_offset.to_le_bytes());
        buf.extend_from_slice(&self.stored_len.to_le_bytes());
        buf.extend_from_slice(&self.raw_len.to_le_bytes());
        buf.extend_from_slice(&self.expire_at.to_le_bytes());
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.relocation_generation.to_le_bytes());
        buf.extend_from_slice(&self.flags.to_le_bytes());
    }

    pub fn decode(data: &[u8]) -> Result<(Self, usize), TitanError> {
        let min_len = 8 + 1 + 1 + 8 + 4 + 4 + 8 + 8 + 4 + 2;
        if data.len() < min_len {
            return Err(TitanError::CorruptedRecord("Index record truncated".into()));
        }

        let mut offset = 0;
        let hash = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let key_len = data[offset] as usize;
        offset += 1;
        if offset + key_len > data.len() {
            return Err(TitanError::CorruptedRecord("Invalid key length in index".into()));
        }
        let key = data[offset..offset + key_len].to_vec();
        offset += key_len;

        if offset >= data.len() {
            return Err(TitanError::CorruptedRecord("Index record truncated at segment id".into()));
        }
        let seg_len = data[offset] as usize;
        offset += 1;
        if offset + seg_len > data.len() {
            return Err(TitanError::CorruptedRecord("Invalid segment length in index".into()));
        }
        let segment_id = String::from_utf8(data[offset..offset + seg_len].to_vec())
            .map_err(|e| TitanError::CorruptedRecord(e.to_string()))?;
        offset += seg_len;

        if offset + 38 > data.len() {
            return Err(TitanError::CorruptedRecord("Index record tail truncated".into()));
        }

        let value_offset = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let stored_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let raw_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let expire_at = i64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let sequence = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let relocation_generation = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let flags = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
        offset += 2;

        Ok((
            IndexRecord {
                hash,
                key,
                segment_id,
                value_offset,
                stored_len,
                raw_len,
                expire_at,
                sequence,
                relocation_generation,
                flags,
            },
            offset,
        ))
    }
}
