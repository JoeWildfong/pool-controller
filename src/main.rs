#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)] // for embassy-executor

use core::future::pending;

use embassy_executor::Spawner;
use embassy_net::udp::PacketMetadata;
use embassy_rp::config::Config;
use net::NetworkDriver;
use static_cell::StaticCell;
use time::clock::Clock;

mod time;

mod blink;
mod net;

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<embassy_rp::peripherals::USB>;
});

#[embassy_executor::main]
async fn main(_s: Spawner) {
    let config = Config::default();
    let p = embassy_rp::init(config);

    let (driver, usb, runner) = NetworkDriver::new(p.USB, Irqs);
    static DRIVER_CELL: StaticCell<NetworkDriver> = StaticCell::new();
    let driver = DRIVER_CELL.init(driver);
    let _ = _s.spawn(net::usb_task(usb));
    let _ = _s.spawn(net::usb_ncm_task(runner));
    let _ = _s.spawn(net::net_task(driver));

    let mut clock = Clock::new();
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let _ = time::adjust_current_time(
        driver.ntp_socket(&mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer),
        &mut clock,
    )
    .await;

    pending().await
}
