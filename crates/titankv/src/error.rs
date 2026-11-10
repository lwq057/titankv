use std::fmt;

#[derive(Debug)]
pub enum TitanError {
    InvalidConfig(String),
    ConfigMismatch(String),
    InvalidKey(String),
    InvalidValue(String),
    QueueFull,
    MemoryLimit,
    IoError(std::io::Error),
    ChecksumError,
    CorruptedRecord(String),
    IndexCorrupted(String),
    RecoveryRequired(String),
    DatabaseLocked,
    DatabaseClosed,
    RecoveryError(String),
    TtlNotSet,
    InternalError(String),
}

impl std::error::Error for TitanError {}

impl fmt::Display for TitanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TitanError::InvalidConfig(s) => write!(f, "INVALID_CONFIG: {}", s),
            TitanError::ConfigMismatch(s) => write!(f, "CONFIG_MISMATCH: {}", s),
            TitanError::InvalidKey(s) => write!(f, "INVALID_KEY: {}", s),
            TitanError::InvalidValue(s) => write!(f, "INVALID_VALUE: {}", s),
            TitanError::QueueFull => write!(f, "QUEUE_FULL: Write queue is full"),
            TitanError::MemoryLimit => write!(f, "MEMORY_LIMIT: Memory budget exceeded"),
            TitanError::IoError(e) => write!(f, "IO_ERROR: {}", e),
            TitanError::ChecksumError => write!(f, "CHECKSUM_ERROR: CRC32C checksum mismatch"),
            TitanError::CorruptedRecord(s) => write!(f, "CORRUPTED_RECORD: {}", s),
            TitanError::IndexCorrupted(s) => write!(f, "INDEX_CORRUPTED: {}", s),
            TitanError::RecoveryRequired(s) => write!(f, "RECOVERY_REQUIRED: {}", s),
            TitanError::DatabaseLocked => write!(f, "DATABASE_LOCKED: Database lock contention"),
            TitanError::DatabaseClosed => write!(f, "DATABASE_CLOSED: Database is closed"),
            TitanError::RecoveryError(s) => write!(f, "RECOVERY_ERROR: {}", s),
            TitanError::TtlNotSet => write!(f, "TTL_NOT_SET: Key has no finite TTL to modify"),
            TitanError::InternalError(s) => write!(f, "INTERNAL_ERROR: {}", s),
        }
    }
}

impl From<std::io::Error> for TitanError {
    fn from(err: std::io::Error) -> Self {
        TitanError::IoError(err)
    }
}

impl TitanError {
    pub fn code(&self) -> &'static str {
        match self {
            TitanError::InvalidConfig(_) => "INVALID_CONFIG",
            TitanError::ConfigMismatch(_) => "CONFIG_MISMATCH",
            TitanError::InvalidKey(_) => "INVALID_KEY",
            TitanError::InvalidValue(_) => "INVALID_VALUE",
            TitanError::QueueFull => "QUEUE_FULL",
            TitanError::MemoryLimit => "MEMORY_LIMIT",
            TitanError::IoError(_) => "IO_ERROR",
            TitanError::ChecksumError => "CHECKSUM_ERROR",
            TitanError::CorruptedRecord(_) => "CORRUPTED_RECORD",
            TitanError::IndexCorrupted(_) => "INDEX_CORRUPTED",
            TitanError::RecoveryRequired(_) => "RECOVERY_REQUIRED",
            TitanError::DatabaseLocked => "DATABASE_LOCKED",
            TitanError::DatabaseClosed => "DATABASE_CLOSED",
            TitanError::RecoveryError(_) => "RECOVERY_ERROR",
            TitanError::TtlNotSet => "TTL_NOT_SET",
            TitanError::InternalError(_) => "INTERNAL_ERROR",
        }
    }
}
