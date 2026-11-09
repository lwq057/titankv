use crate::checksum::{crc32c_calculate, crc32c_verify};
use crate::error::TitanError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtlEvent {
    pub expire_at: i64,
    pub shard_id: u16,
    pub local_bucket_id: u32,
    pub sequence: u64,
    pub key: Vec<u8>,
}

impl TtlEvent {
    pub fn encode(&self, buf: &mut Vec<u8>) {
        let start = buf.len();
        buf.extend_from_slice(&self.expire_at.to_le_bytes());
        buf.extend_from_slice(&self.shard_id.to_le_bytes());
        buf.extend_from_slice(&self.local_bucket_id.to_le_bytes());
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.push(self.key.len() as u8);
        buf.extend_from_slice(&self.key);

        let checksum = crc32c_calculate(&buf[start..]);
        buf.extend_from_slice(&checksum.to_le_bytes());
    }

    pub fn decode(data: &[u8]) -> Result<(Self, usize), TitanError> {
        let min_len = 8 + 2 + 4 + 8 + 1 + 4;
        if data.len() < min_len {
            return Err(TitanError::CorruptedRecord("Incomplete TTL event".into()));
        }

        let expire_at = i64::from_le_bytes(data[0..8].try_into().unwrap());
        let shard_id = u16::from_le_bytes(data[8..10].try_into().unwrap());
        let local_bucket_id = u32::from_le_bytes(data[10..14].try_into().unwrap());
        let sequence = u64::from_le_bytes(data[14..22].try_into().unwrap());
        let key_len = data[22] as usize;

        let payload_len = 23 + key_len;
        if data.len() < payload_len + 4 {
            return Err(TitanError::CorruptedRecord("TTL event truncated".into()));
        }

        let expected_crc = u32::from_le_bytes(data[payload_len..payload_len + 4].try_into().unwrap());
        if !crc32c_verify(&data[0..payload_len], expected_crc) {
            return Err(TitanError::ChecksumError);
        }

        let key = data[23..payload_len].to_vec();
        Ok((
            Self {
                expire_at,
                shard_id,
                local_bucket_id,
                sequence,
                key,
            },
            payload_len + 4,
        ))
    }
}
