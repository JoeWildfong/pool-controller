use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

pub mod clock;
use clock::Clock;
use embassy_time::Duration;

use crate::platform::NtpSocket;

#[derive(Debug)]
pub enum AdjustCurrentTimeError {
    Timeout,
    Ntp,
}

impl From<sntpc::Error> for AdjustCurrentTimeError {
    fn from(_value: sntpc::Error) -> Self {
        Self::Ntp
    }
}

impl From<embassy_time::TimeoutError> for AdjustCurrentTimeError {
    fn from(_value: embassy_time::TimeoutError) -> Self {
        Self::Timeout
    }
}

pub async fn adjust_current_time(
    socket: &NtpSocket,
    clock: &mut Clock,
) -> Result<(), AdjustCurrentTimeError> {
    use sntpc::NtpContext;

    let context = NtpContext::new(clock.get_timestamp_gen());
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(207, 210, 46, 249), 123));
    let res = embassy_time::with_timeout(
        Duration::from_secs(10),
        sntpc::get_time(addr, socket, context),
    )
    .await??;
    clock.inject_ntp(res);
    Ok(())
}
