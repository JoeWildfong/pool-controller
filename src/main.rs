#![no_std]
#![feature(impl_trait_in_assoc_type)] // for embassy-executor
#![cfg_attr(not(feature = "sim"), no_main)]

use core::future::pending;

use embassy_executor::Spawner;

#[cfg(not(feature = "sim"))]
mod blink;
#[cfg(not(feature = "sim"))]
mod wifi;
#[cfg(not(feature = "sim"))]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
#[cfg(not(feature = "sim"))]
embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
});

#[cfg(feature = "sim")]
extern crate std;


#[embassy_executor::main]
async fn main(_s: Spawner) {
    #[cfg(not(feature = "sim"))]
    {
        use embassy_rp::{gpio::{Level, Output}, config::Config, pio::Pio};
        use static_cell::StaticCell;

        let fw = include_bytes!("../firmware/43439A0.bin");
        let clm = include_bytes!("../firmware/43439A0_clm.bin");
        let config = Config::default();
        let p = embassy_rp::init(config);

        let pwr = Output::new(p.PIN_23, Level::Low);
        let cs = Output::new(p.PIN_25, Level::High);
        let clk = p.PIN_29;
        let dio = p.PIN_24;

        let mut pio0 = Pio::new(p.PIO0, Irqs);
        let cyw_spi = cyw43_pio::PioSpi::new(&mut pio0.common, pio0.sm0, pio0.irq0, cs, dio, clk, p.DMA_CH0);
        let state = {
            static CELL: StaticCell<cyw43::State> = StaticCell::new();
            CELL.uninit().write(cyw43::State::new())
        };
        let (_net_device, mut control, runner) = cyw43::new(state, pwr, cyw_spi, fw).await;
        let _ = _s.spawn(wifi::wifi_task(runner));
        control.init(clm).await;
        let _ = _s.spawn(blink::blink(control));
        pending().await
    }
}
