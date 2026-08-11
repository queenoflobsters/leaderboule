use std::time::{SystemTime, UNIX_EPOCH};

pub const THIRTY_DAYS_IN_SECS: u64 = 30 * 24 * 3600;

pub fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap()
}
