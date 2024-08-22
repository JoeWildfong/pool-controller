use core::fmt::Debug;
use sntpc::async_impl::NtpUdpSocket;

pub mod clock;
use clock::Clock;

pub async fn adjust_current_time<S: NtpUdpSocket + Debug>(
    socket: S,
    clock: &mut Clock,
) -> sntpc::Result<()> {
    use sntpc::NtpContext;

    let context = NtpContext::new(*clock);
    let addr = no_std_net::SocketAddrV4::new(no_std_net::Ipv4Addr::new(207, 210, 46, 249), 123);
    let res = sntpc::async_impl::get_time(addr, socket, context).await?;
    clock.inject_ntp(res);
    Ok(())
}
