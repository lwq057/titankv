use crate::concurrency::process_lock::{ProcessFileGuard, ProcessLockManager};
use crate::error::TitanError;
use std::sync::Arc;

pub struct DatabaseLifecycle {
    pub process_lock: Arc<ProcessLockManager>,
    pub db_lock_guard: Option<ProcessFileGuard>,
    pub is_primary_recovered: bool,
}

impl DatabaseLifecycle {
    pub fn initialize(process_lock: Arc<ProcessLockManager>) -> Result<Self, TitanError> {
        // Step 1: acquire recovery.lock exclusive
        let _recovery_guard = process_lock.acquire_recovery_lock_exclusive()?;

        // Step 2: Try to acquire database.lock exclusive
        let (db_guard, is_primary) = match process_lock.try_acquire_database_exclusive()? {
            Some(_exclusive_guard) => {
                // We are the first/sole active process! We can perform recovery.
                // After recovery is done, downgrade to shared lock.
                drop(_exclusive_guard);
                let shared = process_lock.acquire_database_shared()?;
                (shared, true)
            }
            None => {
                // Another process is already active. Skip recovery and acquire shared lock.
                let shared = process_lock.acquire_database_shared()?;
                (shared, false)
            }
        };

        // Step 3: _recovery_guard is dropped here, releasing recovery.lock
        Ok(Self {
            process_lock,
            db_lock_guard: Some(db_guard),
            is_primary_recovered: is_primary,
        })
    }

    pub fn close(&mut self) {
        self.db_lock_guard = None;
    }
}
