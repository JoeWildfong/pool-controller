use embassy_net::StackResources;
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
use rand_core::RngCore as _;
use static_cell::StaticCell;

pub const MTU: usize = 1514;

pub type UsbDriver = Driver<'static, embassy_rp::peripherals::USB>;

#[embassy_executor::task]
pub async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

#[embassy_executor::task]
pub async fn usb_ncm_task(class: Runner<'static, UsbDriver, MTU>) -> ! {
    class.run().await
}

#[embassy_executor::task]
pub async fn net_task(mut driver: embassy_net::Runner<'static, Device<'static, MTU>>) -> ! {
    driver.run().await
}

pub struct NetworkDriver {
    inner: embassy_net::Stack<'static>,
}

impl NetworkDriver {
    pub fn new<B, P>(
        peripheral: P,
        irq: B,
    ) -> (
        Self,
        UsbDevice<'static, UsbDriver>,
        Runner<'static, Driver<'static, USB>, MTU>,
        embassy_net::Runner<'static, Device<'static, MTU>>,
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
        let (usb_ncm_runner, device) = class
            .into_embassy_net_device::<MTU, 4, 4>(NET_STATE.init(NetState::new()), our_mac_addr);

        let config = embassy_net::Config::dhcpv4(Default::default());

        // Generate random seed
        let mut rng = RoscRng;
        let seed = rng.next_u64();

        static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
        let (stack, net_runner) =
            embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);

        let driver = Self { inner: stack };
        (driver, usb, usb_ncm_runner, net_runner)
    }

    pub fn stack(&self) -> embassy_net::Stack<'static> {
        self.inner
    }
}
