use embedded_graphics::{pixelcolor::Rgb565, prelude::DrawTarget};
use sntpc::NtpUdpSocket;

pub type Screen = impl DrawTarget<Color = Rgb565, Error: core::fmt::Debug>;
pub type NtpSocket = impl NtpUdpSocket;
pub type Pump = impl PumpOutput;

pub trait PumpOutput {
    fn set_running(&mut self, running: bool);
}

#[cfg(feature = "device")]
pub mod device {
    use crate::{
        net::{NetworkDriver, UsbDriver, MTU},
        Irqs,
    };
    use embassy_net::{
        udp::{PacketMetadata, UdpSocket},
        IpListenEndpoint,
    };
    use embassy_rp::{
        gpio::{Level, Output},
        peripherals::{PIN_0, PIN_16, PIN_17, PIN_18, PIN_19, PIN_2, SPI0, USB},
        spi::{Config as SpiConfig, Spi},
        usb::Driver,
    };
    use embassy_time::Delay;
    use embassy_usb::{
        class::cdc_ncm::embassy_net::{Device, Runner},
        UsbDevice,
    };
    use embedded_hal_bus::spi::ExclusiveDevice;
    use mipidsi::{
        interface::SpiInterface,
        options::{Orientation, Rotation},
    };
    use static_cell::{ConstStaticCell, StaticCell};

    use super::{NtpSocket, Pump, PumpOutput, Screen};

    #[define_opaque(Screen)]
    // pub fn real_screen(spi: ScreenSpi, dc: ScreenDc) -> Screen {
    pub fn screen(
        spi: SPI0,
        clk: PIN_18,
        mosi: PIN_19,
        miso: PIN_16,
        cs: PIN_17,
        dc: PIN_2,
    ) -> Screen {
        let cs = Output::new(cs, Level::High);
        let spi = ExclusiveDevice::new(
            Spi::new_blocking(spi, clk, mosi, miso, SpiConfig::default()),
            cs,
            Delay,
        )
        .unwrap();
        let dc = Output::new(dc, Level::Low);
        static BUFFER: ConstStaticCell<[u8; 256]> = ConstStaticCell::new([0; 256]);
        let interface = SpiInterface::new(spi, dc, BUFFER.take());
        mipidsi::Builder::new(mipidsi::models::ST7789, interface)
            .display_size(135, 240)
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            .init(&mut Delay)
            .unwrap()
    }

    #[define_opaque(NtpSocket)]
    pub fn ntp(
        usb: USB,
    ) -> (
        NtpSocket,
        UsbDevice<'static, UsbDriver>,
        Runner<'static, Driver<'static, USB>, MTU>,
        embassy_net::Runner<'static, Device<'static, MTU>>,
    ) {
        let (driver, usb, usb_ncm_runner, net_runner) = NetworkDriver::new(usb, Irqs);
        static DRIVER_CELL: StaticCell<NetworkDriver> = StaticCell::new();
        let driver = DRIVER_CELL.init(driver);
        static RX_BUFFER: ConstStaticCell<[u8; 4096]> = ConstStaticCell::new([0; 4096]);
        static TX_BUFFER: ConstStaticCell<[u8; 4096]> = ConstStaticCell::new([0; 4096]);
        static RX_META: ConstStaticCell<[PacketMetadata; 16]> =
            ConstStaticCell::new([PacketMetadata::EMPTY; 16]);
        static TX_META: ConstStaticCell<[PacketMetadata; 16]> =
            ConstStaticCell::new([PacketMetadata::EMPTY; 16]);
        let mut socket = UdpSocket::new(
            driver.stack(),
            RX_META.take(),
            RX_BUFFER.take(),
            TX_META.take(),
            TX_BUFFER.take(),
        );
        socket.bind(IpListenEndpoint::from(0)).unwrap();
        (socket, usb, usb_ncm_runner, net_runner)
    }

    #[define_opaque(Pump)]
    pub fn pump(pin: PIN_0) -> Pump {
        Output::new(pin, Level::Low)
    }

    impl PumpOutput for Output<'_> {
        fn set_running(&mut self, running: bool) {
            self.set_level(Level::from(running));
        }
    }
}

#[cfg(feature = "sim")]
pub mod sim {
    use core::{net::Ipv4Addr, time::Duration};
    use std::{
        net::UdpSocket,
        sync::mpsc::{channel, Sender},
    };

    use embedded_graphics::{
        pixelcolor::Rgb565,
        prelude::{Dimensions, DrawTarget, Size},
    };
    use embedded_graphics_simulator::{OutputSettings, SimulatorDisplay, SimulatorEvent, Window};

    use super::{NtpSocket, Pump, PumpOutput, Screen};

    #[define_opaque(Screen)]
    pub fn screen() -> Screen {
        let screen = SimulatorDisplay::<Rgb565>::new(Size::new(240, 135));
        let (send, recv) = channel();
        let starting_screen = screen.clone();
        std::thread::spawn(move || {
            let mut window = Window::new("embedded-graphics output", &OutputSettings::default());
            window.update(&starting_screen);
            loop {
                for event in window.events() {
                    if event == SimulatorEvent::Quit {
                        std::process::exit(0);
                    }
                }
                while let Ok(surface) = recv.try_recv() {
                    window.update(&surface);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        WindowDisplay {
            inner: screen,
            send,
        }
    }

    struct WindowDisplay {
        inner: SimulatorDisplay<Rgb565>,
        send: Sender<SimulatorDisplay<Rgb565>>,
    }

    impl Dimensions for WindowDisplay {
        fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
            self.inner.bounding_box()
        }
    }

    impl DrawTarget for WindowDisplay {
        type Color = Rgb565;
        type Error = core::convert::Infallible;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
        {
            self.inner.draw_iter(pixels)?;
            self.send.send(self.inner.clone()).unwrap();
            Ok(())
        }
    }

    #[define_opaque(NtpSocket)]
    pub fn ntp() -> NtpSocket {
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap()
    }

    #[define_opaque(Pump)]
    pub fn pump() -> Pump {
        FakePump
    }

    struct FakePump;

    impl PumpOutput for FakePump {
        fn set_running(&mut self, running: bool) {
            println!("running: {running}");
        }
    }
}
