use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

pub mod clock;
use clock::Clock;

use crate::platform::NtpSocket;

pub async fn adjust_current_time(
    socket: &NtpSocket,
    clock: &mut Clock,
) -> sntpc::Result<()> {
    use sntpc::NtpContext;

    let context = NtpContext::new(clock.get_timestamp_gen());
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(207, 210, 46, 249), 123));
    let res = sntpc::get_time(addr, socket, context).await?;
    clock.inject_ntp(res);
    Ok(())
}
