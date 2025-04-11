use embassy_time::{Duration, Instant};
use jiff::{Timestamp, Zoned};
use sntpc::{NtpResult, NtpTimestampGenerator};

use crate::TORONTO_TZ;

#[derive(Debug, Default, Clone, Copy)]
pub struct Clock {
    offset: Duration,
}

impl Clock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_unix_time(&self) -> Instant {
        Instant::now() + self.offset
    }

    pub fn get_toronto_time(&self) -> Zoned {
        Zoned::new(
            Timestamp::from_microsecond(self.get_unix_time().as_micros().try_into().unwrap())
                .unwrap(),
            TORONTO_TZ.clone(),
        )
    }

    pub fn inject_ntp(&mut self, measurement: NtpResult) {
        match measurement.offset() {
            i @ 0.. => self.offset += Duration::from_millis(i.unsigned_abs()),
            i @ ..0 => self.offset -= Duration::from_millis(i.unsigned_abs()),
        }
    }
}

impl NtpTimestampGenerator for Clock {
    fn init(&mut self) {}

    fn timestamp_sec(&self) -> u64 {
        self.get_unix_time().as_secs()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.get_unix_time().as_micros() % 1_000_000) as u32
    }
}
