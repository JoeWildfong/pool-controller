#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)] // for embassy-executor

use embassy_executor::Spawner;
use embassy_net::udp::PacketMetadata;
use embassy_rp::{
    config::Config,
    gpio::{Level, Output},
};
use embassy_time::{Delay, Duration};
use embedded_hal_async::delay::DelayNs;
use jiff::{civil::Time, tz::TimeZone, Zoned};
use net::NetworkDriver;
use static_cell::StaticCell;
use time::clock::Clock;

mod blink;
mod net;
mod time;

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo<'_>) -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<embassy_rp::peripherals::USB>;
});

static TORONTO_TZ: TimeZone = jiff::tz::get!("America/Toronto");

struct RunningState {
    running: bool,
    duration: Duration,
}

impl RunningState {
    const ON_TIME: Time = jiff::civil::time(9, 0, 0, 0);
    const OFF_TIME: Time = jiff::civil::time(12, 0, 0, 0);

    pub fn from_timestamp(now: &Zoned) -> Self {
        if (Self::ON_TIME..Self::OFF_TIME).contains(&now.time()) {
            let off_zoned = now.with().time(Self::OFF_TIME).build().unwrap();
            let delta = Duration::from_millis(
                (&off_zoned - now)
                    .get_milliseconds()
                    .try_into()
                    .unwrap_or(0),
            );
            Self {
                running: true,
                duration: delta,
            }
        } else {
            let on_zoned = now
                .with()
                .date(now.date().tomorrow().unwrap())
                .time(Self::ON_TIME)
                .build()
                .unwrap();
            let delta =
                Duration::from_millis((&on_zoned - now).get_milliseconds().try_into().unwrap_or(0));
            Self {
                running: false,
                duration: delta,
            }
        }
    }
}

#[embassy_executor::main]
async fn main(_s: Spawner) {
    let config = Config::default();
    let p = embassy_rp::init(config);
    let mut gpio = Output::new(p.PIN_0, Level::Low);

    let (driver, usb, runner) = NetworkDriver::new(p.USB, Irqs);
    static DRIVER_CELL: StaticCell<NetworkDriver> = StaticCell::new();
    let driver = DRIVER_CELL.init(driver);
    _s.spawn(net::usb_task(usb)).unwrap();
    _s.spawn(net::usb_ncm_task(runner)).unwrap();
    _s.spawn(net::net_task(driver)).unwrap();

    let mut clock = Clock::new();
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let ntp_socket = driver.ntp_socket(&mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);

    let mut last_ntp = embassy_time::Instant::now();
    loop {
        let monotonic_now = embassy_time::Instant::now();
        if (monotonic_now - last_ntp).as_secs() > 3600 {
            match time::adjust_current_time(&ntp_socket, &mut clock).await {
                Ok(()) => last_ntp = monotonic_now,
                Err(_e) => {}
            }
        }
        let wall_now = clock.get_toronto_time();
        let current_state = RunningState::from_timestamp(&wall_now);
        gpio.set_level(Level::from(current_state.running));
        Delay
            .delay_ms(current_state.duration.as_millis().try_into().unwrap())
            .await;
    }
}
