use crate::checksum::{crc32c_calculate, crc32c_verify};
use crate::error::TitanError;

pub const LOG_MAGIC: [u8; 4] = *b"TLOG";
pub const LOG_COMMIT_MARKER: [u8; 4] = *b"TEND";
pub const LOG_VERSION: u8 = 1;

pub const RECORD_TYPE_PUT: u8 = 1;
pub const RECORD_TYPE_TOMBSTONE: u8 = 2;

pub const LOG_HEADER_SIZE: usize = 4 + 1 + 1 + 2 + 4 + 8 + 8 + 4 + 2 + 4 + 4; // 42 bytes
pub const LOG_TRAILER_SIZE: usize = 4 + 4; // 8 bytes

#[derive(Debug, Clone)]
pub struct LogRecordHeader {
    pub record_type: u8,
    pub flags: u16,
    pub record_length: u32,
    pub sequence: u64,
    pub expire_at: i64,
    pub relocation_generation: u32,
    pub key_len: u16,
    pub raw_value_len: u32,
    pub stored_value_len: u32,
}

pub fn encode_log_record(
    record_type: u8,
    flags: u16,
    sequence: u64,
    expire_at: i64,
    relocation_generation: u32,
    key: &[u8],
    raw_value_len: u32,
    stored_value: &[u8],
    buf: &mut Vec<u8>,
) {
    let key_len = key.len() as u16;
    let stored_value_len = stored_value.len() as u32;
    let total_len = LOG_HEADER_SIZE + (key_len as usize) + (stored_value_len as usize) + LOG_TRAILER_SIZE;

    let start_idx = buf.len();

    // Header
    buf.extend_from_slice(&LOG_MAGIC);
    buf.push(LOG_VERSION);
    buf.push(record_type);
    buf.extend_from_slice(&flags.to_le_bytes());
    buf.extend_from_slice(&(total_len as u32).to_le_bytes());
    buf.extend_from_slice(&sequence.to_le_bytes());
    buf.extend_from_slice(&expire_at.to_le_bytes());
    buf.extend_from_slice(&relocation_generation.to_le_bytes());
    buf.extend_from_slice(&key_len.to_le_bytes());
    buf.extend_from_slice(&raw_value_len.to_le_bytes());
    buf.extend_from_slice(&stored_value_len.to_le_bytes());

    // Payload
    buf.extend_from_slice(key);
    buf.extend_from_slice(stored_value);

    // CRC32C over everything up to payload end
    let payload_bytes = &buf[start_idx..];
    let checksum = crc32c_calculate(payload_bytes);
    buf.extend_from_slice(&checksum.to_le_bytes());
    buf.extend_from_slice(&LOG_COMMIT_MARKER);
}

pub fn decode_log_record<'a>(
    data: &'a [u8],
) -> Result<(LogRecordHeader, &'a [u8], &'a [u8], usize), TitanError> {
    if data.len() < LOG_HEADER_SIZE + LOG_TRAILER_SIZE {
        return Err(TitanError::CorruptedRecord("Incomplete log record".into()));
    }

    if &data[0..4] != &LOG_MAGIC {
        return Err(TitanError::CorruptedRecord("Invalid log magic".into()));
    }

    let version = data[4];
    if version != LOG_VERSION {
        return Err(TitanError::CorruptedRecord(format!(
            "Unsupported log version: {}",
            version
        )));
    }

    let record_type = data[5];
    let flags = u16::from_le_bytes(data[6..8].try_into().unwrap());
    let record_length = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;

    if data.len() < record_length {
        return Err(TitanError::CorruptedRecord("Log record shorter than header specifies".into()));
    }

    let sequence = u64::from_le_bytes(data[12..20].try_into().unwrap());
    let expire_at = i64::from_le_bytes(data[20..28].try_into().unwrap());
    let relocation_generation = u32::from_le_bytes(data[28..32].try_into().unwrap());
    let key_len = u16::from_le_bytes(data[32..34].try_into().unwrap()) as usize;
    let raw_value_len = u32::from_le_bytes(data[34..38].try_into().unwrap());
    let stored_value_len = u32::from_le_bytes(data[38..42].try_into().unwrap()) as usize;

    let payload_end = LOG_HEADER_SIZE + key_len + stored_value_len;
    if payload_end + LOG_TRAILER_SIZE != record_length {
        return Err(TitanError::CorruptedRecord("Log record length mismatch".into()));
    }

    // Verify commit marker
    if &data[record_length - 4..record_length] != &LOG_COMMIT_MARKER {
        return Err(TitanError::CorruptedRecord("Missing log commit marker".into()));
    }

    // Verify CRC32C
    let expected_crc = u32::from_le_bytes(data[payload_end..payload_end + 4].try_into().unwrap());
    if !crc32c_verify(&data[0..payload_end], expected_crc) {
        return Err(TitanError::ChecksumError);
    }

    let key = &data[LOG_HEADER_SIZE..LOG_HEADER_SIZE + key_len];
    let value = &data[LOG_HEADER_SIZE + key_len..payload_end];

    Ok((
        LogRecordHeader {
            record_type,
            flags,
            record_length: record_length as u32,
            sequence,
            expire_at,
            relocation_generation,
            key_len: key_len as u16,
            raw_value_len,
            stored_value_len: stored_value_len as u32,
        },
        key,
        value,
        record_length,
    ))
}
