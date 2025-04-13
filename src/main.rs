#![cfg_attr(feature = "device", no_std)]
#![cfg_attr(feature = "device", no_main)]
#![feature(impl_trait_in_assoc_type)] // for embassy-executor
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embassy_time::Delay;
use embedded_hal_async::delay::DelayNs;
use jiff::{civil::Time, tz::TimeZone, Unit, Zoned};
use screen::ScreenState;
use static_cell::ConstStaticCell;
use time::clock::Clock;

#[cfg(feature = "device")]
mod net;
mod platform;
use platform::{NtpSocket, Pump, PumpOutput, Screen};
mod screen;
mod time;

#[cfg(not(any(feature = "device", feature = "sim")))]
core::compile_error!("you must enable one of the `device` and `sim` features");
#[cfg(all(feature = "device", not(all(target_arch = "arm", target_os = "none"))))]
core::compile_error!("feature `device` is only supported on rp2040");

#[cfg(feature = "device")]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo<'_>) -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

#[cfg(feature = "device")]
embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<embassy_rp::peripherals::USB>;
});

static TORONTO_TZ: TimeZone = jiff::tz::get!("America/Toronto");

struct RunningState {
    running: bool,
    until: jiff::Zoned,
}

impl RunningState {
    const ON_TIME: Time = jiff::civil::time(9, 0, 0, 0);
    const OFF_TIME: Time = jiff::civil::time(12, 0, 0, 0);

    pub fn from_wall_time(now: &Zoned) -> Self {
        if (Self::ON_TIME..Self::OFF_TIME).contains(&now.time()) {
            let off_zoned = now.with().time(Self::OFF_TIME).build().unwrap();
            Self {
                running: true,
                until: off_zoned,
            }
        } else {
            let date = if now.time() < Self::ON_TIME {
                now.date()
            } else {
                now.date().tomorrow().unwrap()
            };
            let on_zoned = now
                .with()
                .date(date)
                .time(Self::ON_TIME)
                .build()
                .unwrap();

            Self {
                running: false,
                until: on_zoned,
            }
        }
    }
}

fn setup_platform(_s: &Spawner) -> (Screen, NtpSocket, Pump) {
    #[cfg(feature = "device")]
    {
        use embassy_rp::config::Config;

        let config = Config::default();
        let p = embassy_rp::init(config);

        let screen = platform::device::screen(p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.PIN_17, p.PIN_2);

        let (ntp_socket, usb, usb_ncm_runner, net_runner) = platform::device::ntp(p.USB);
        _s.spawn(net::usb_task(usb)).unwrap();
        _s.spawn(net::usb_ncm_task(usb_ncm_runner)).unwrap();
        _s.spawn(net::net_task(net_runner)).unwrap();

        let pump = platform::device::pump(p.PIN_0);

        (screen, ntp_socket, pump)
    }
    #[cfg(not(feature = "device"))]
    {
        (platform::sim::screen(), platform::sim::ntp(), platform::sim::pump())
    }
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    let (screen, ntp_socket, mut pump) = setup_platform(&s);
    static SCREEN_SIGNAL: ConstStaticCell<Signal<NoopRawMutex, screen::ScreenState>> = ConstStaticCell::new(Signal::new());
    let screen_signal = SCREEN_SIGNAL.take();
    s.spawn(screen::drive_screen(screen_signal, screen)).unwrap();

    let mut clock = Clock::new();

    time::adjust_current_time(&ntp_socket, &mut clock).await.unwrap();
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
        let current_state = RunningState::from_wall_time(&wall_now);
        pump.set_running(current_state.running);
        let show = match current_state.running {
            false => ScreenState::OffUntil { hour: current_state.until.hour(), min: current_state.until.minute() },
            true => ScreenState::OnUntil { hour: current_state.until.hour(), min: current_state.until.minute() },
        };
        screen_signal.signal(show);
        let delay_ms = wall_now.until((Unit::Millisecond, &current_state.until)).unwrap().get_milliseconds().try_into().unwrap_or(0);
        Delay
            .delay_ms(delay_ms)
            .await;
    }
}
