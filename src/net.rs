use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    IpEndpoint, Stack, StackResources,
};
use embassy_rp::{
    clocks::RoscRng,
    interrupt::typelevel::Binding,
    peripherals::USB,
    usb::{Driver, InterruptHandler},
    Peripheral,
};
use embassy_usb::{
    class::cdc_ncm::{
        embassy_net::{Device, Runner, State as NetState},
        CdcNcmClass, State,
    },
    Builder, Config, UsbDevice,
};
use no_std_net::{SocketAddr, ToSocketAddrs};
use rand_core::RngCore as _;
use sntpc::async_impl::NtpUdpSocket;
use static_cell::StaticCell;
use thiserror_no_std::Error;

const MTU: usize = 1514;

type UsbDriver = Driver<'static, embassy_rp::peripherals::USB>;

#[embassy_executor::task]
pub async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

#[embassy_executor::task]
pub async fn usb_ncm_task(class: Runner<'static, UsbDriver, MTU>) -> ! {
    class.run().await
}

#[embassy_executor::task]
pub async fn net_task(driver: &'static NetworkDriver) -> ! {
    driver.inner.run().await
}

pub struct NetworkDriver {
    inner:
        &'static embassy_net::Stack<embassy_usb::class::cdc_ncm::embassy_net::Device<'static, MTU>>,
}

impl NetworkDriver {
    pub fn new<B, P>(
        peripheral: P,
        irq: B,
    ) -> (
        Self,
        UsbDevice<'static, UsbDriver>,
        Runner<'static, Driver<'static, USB>, MTU>,
    )
    where
        B: Binding<<USB as embassy_rp::usb::Instance>::Interrupt, InterruptHandler<USB>>,
        P: Peripheral<P = USB> + 'static,
    {
        // wtf... copied from https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/usb_ethernet.rs @ 59cb153

        // Create the driver, from the HAL.
        let driver = Driver::new(peripheral, irq);

        // Create embassy-usb Config
        let mut config = Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Embassy");
        config.product = Some("USB-Ethernet example");
        config.serial_number = Some("12345678");
        config.max_power = 100;
        config.max_packet_size_0 = 64;

        // Required for Windows support.
        config.composite_with_iads = true;
        config.device_class = 0xEF;
        config.device_sub_class = 0x02;
        config.device_protocol = 0x01;

        // Create embassy-usb DeviceBuilder using the driver and config.
        static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();
        let mut builder = Builder::new(
            driver,
            config,
            &mut CONFIG_DESC.init([0; 256])[..],
            &mut BOS_DESC.init([0; 256])[..],
            &mut [], // no msos descriptors
            &mut CONTROL_BUF.init([0; 128])[..],
        );

        // Our MAC addr.
        let our_mac_addr = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC];
        // Host's MAC addr. This is the MAC the host "thinks" its USB-to-ethernet adapter has.
        let host_mac_addr = [0x88, 0x88, 0x88, 0x88, 0x88, 0x88];

        // Create classes on the builder.
        static STATE: StaticCell<State> = StaticCell::new();
        let class = CdcNcmClass::new(&mut builder, STATE.init(State::new()), host_mac_addr, 64);

        // Build the builder.
        let usb = builder.build();

        static NET_STATE: StaticCell<NetState<MTU, 4, 4>> = StaticCell::new();
        let (runner, device) = class
            .into_embassy_net_device::<MTU, 4, 4>(NET_STATE.init(NetState::new()), our_mac_addr);

        let config = embassy_net::Config::dhcpv4(Default::default());

        // Generate random seed
        let mut rng = RoscRng;
        let seed = rng.next_u64();

        // Init network stack
        static STACK: StaticCell<Stack<Device<'static, MTU>>> = StaticCell::new();
        static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
        let stack = &*STACK.init(Stack::new(
            device,
            config,
            RESOURCES.init(StackResources::new()),
            seed,
        ));

        let driver = Self { inner: stack };
        (driver, usb, runner)
    }

    pub fn ntp_socket<'a>(
        &'a self,
        rx_meta: &'a mut [PacketMetadata],
        rx_buffer: &'a mut [u8],
        tx_meta: &'a mut [PacketMetadata],
        tx_buffer: &'a mut [u8],
    ) -> NtpSocket<'a> {
        NtpSocket {
            inner: UdpSocket::new(self.inner, rx_meta, rx_buffer, tx_meta, tx_buffer),
        }
    }
}

pub struct NtpSocket<'a> {
    inner: embassy_net::udp::UdpSocket<'a>,
}

impl core::fmt::Debug for NtpSocket<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NtpSocket {{..}}")
    }
}

// copied mostly from https://github.com/vpikulik/sntpc_embassy/blob/main/src/time.rs @ 930f2e8

#[derive(Error, Debug)]
pub enum SntpcError {
    #[error("to_socket_addrs")]
    ToSocketAddrs,
    #[error("no addr")]
    NoAddr,
    #[error("udp send")]
    UdpSend,
    #[error("dns query error")]
    DnsQuery(#[from] embassy_net::dns::Error),
    #[error("dns query error")]
    DnsEmptyResponse,
    #[error("sntc")]
    Sntc(#[from] sntpc::Error),
    #[error("can not parse ntp response")]
    BadNtpResponse,
}

impl From<SntpcError> for sntpc::Error {
    fn from(err: SntpcError) -> Self {
        match err {
            SntpcError::ToSocketAddrs => Self::AddressResolve,
            SntpcError::NoAddr => Self::AddressResolve,
            SntpcError::UdpSend => Self::Network,
            _ => todo!(),
        }
    }
}

impl<'a> NtpUdpSocket for NtpSocket<'a> {
    async fn send_to<T: ToSocketAddrs + Send>(&self, buf: &[u8], addr: T) -> sntpc::Result<usize> {
        let mut addr_iter = addr
            .to_socket_addrs()
            .map_err(|_| SntpcError::ToSocketAddrs)?;
        let addr = addr_iter.next().ok_or(SntpcError::NoAddr)?;
        self.inner
            .send_to(buf, sock_addr_to_emb_endpoint(addr))
            .await
            .map_err(|_| SntpcError::UdpSend)
            .unwrap();
        Ok(buf.len())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> sntpc::Result<(usize, SocketAddr)> {
        match self.inner.recv_from(buf).await {
            Ok((size, ip_endpoint)) => Ok((size, emb_endpoint_to_sock_addr(ip_endpoint))),
            Err(_) => panic!("not exp"),
        }
    }
}

fn emb_endpoint_to_sock_addr(endpoint: IpEndpoint) -> SocketAddr {
    let port = endpoint.port;
    let addr = match endpoint.addr {
        embassy_net::IpAddress::Ipv4(ipv4) => {
            let octets = ipv4.as_bytes();
            let ipv4_addr = no_std_net::Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]);
            no_std_net::IpAddr::V4(ipv4_addr)
        }
    };
    SocketAddr::new(addr, port)
}

fn sock_addr_to_emb_endpoint(sock_addr: SocketAddr) -> IpEndpoint {
    let port = sock_addr.port();
    let addr = match sock_addr {
        SocketAddr::V4(addr) => {
            let octets = addr.ip().octets();
            embassy_net::IpAddress::v4(octets[0], octets[1], octets[2], octets[3])
        }
        _ => todo!(),
    };
    IpEndpoint::new(addr, port)
}
