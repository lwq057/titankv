use parking_lot::Mutex;

pub struct LaneManager {
    lane_locks: Vec<Mutex<()>>,
}

impl LaneManager {
    pub fn new(total_lanes: usize) -> Self {
        let mut lane_locks = Vec::with_capacity(total_lanes);
        for _ in 0..total_lanes {
            lane_locks.push(Mutex::new(()));
        }
        Self { lane_locks }
    }

    #[inline]
    pub fn lock_lane(&self, global_lane_id: usize) -> parking_lot::MutexGuard<'_, ()> {
        self.lane_locks[global_lane_id % self.lane_locks.len()].lock()
    }
}
