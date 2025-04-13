use core::time::Duration;

use embassy_time::Instant;
use jiff::{Timestamp, Zoned};
use sntpc::{NtpResult, NtpTimestampGenerator};

use crate::TORONTO_TZ;

#[derive(Debug, Default, Clone)]
pub struct Clock {
    startup_time: jiff::Zoned,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            startup_time: jiff::Zoned::default().with_time_zone(TORONTO_TZ.clone())
        }
    }

    pub fn get_timestamp(&self) -> Timestamp {
        self.get_toronto_time().timestamp()
    }

    pub fn get_toronto_time(&self) -> Zoned {
        self.startup_time.clone().saturating_add(Duration::from_micros(Instant::now().as_micros()))
    }

    pub fn inject_ntp(&mut self, measurement: NtpResult) {
        self.startup_time += jiff::SignedDuration::from_micros(measurement.offset);
    }

    pub fn get_timestamp_gen(&self) -> TimestampGen<'_> {
        TimestampGen { now: Timestamp::default(), clock: self }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimestampGen<'a> {
    now: Timestamp,
    clock: &'a Clock,
}

impl NtpTimestampGenerator for TimestampGen<'_> {
    fn init(&mut self) {
        self.now = self.clock.get_timestamp()
    }

    fn timestamp_sec(&self) -> u64 {
        self.now.as_second().try_into().unwrap()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        self.now.subsec_microsecond().try_into().unwrap()
    }
}
