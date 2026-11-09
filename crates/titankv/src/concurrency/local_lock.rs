use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub const LOCAL_LOCK_STRIPES: usize = 65536;

pub struct StripedLocalLock {
    locks: Vec<RwLock<()>>,
}

impl StripedLocalLock {
    pub fn new() -> Self {
        let mut locks = Vec::with_capacity(LOCAL_LOCK_STRIPES);
        for _ in 0..LOCAL_LOCK_STRIPES {
            locks.push(RwLock::new(()));
        }
        Self { locks }
    }

    #[inline]
    fn stripe_index(&self, global_bucket_id: usize) -> usize {
        global_bucket_id % LOCAL_LOCK_STRIPES
    }

    #[inline]
    pub fn read(&self, global_bucket_id: usize) -> RwLockReadGuard<'_, ()> {
        self.locks[self.stripe_index(global_bucket_id)].read()
    }

    #[inline]
    pub fn write(&self, global_bucket_id: usize) -> RwLockWriteGuard<'_, ()> {
        self.locks[self.stripe_index(global_bucket_id)].write()
    }
}
