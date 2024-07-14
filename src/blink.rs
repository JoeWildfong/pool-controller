use cyw43::Control;
use embassy_time::Delay;
use embedded_hal_async::delay::DelayNs;

#[embassy_executor::task]
pub async fn blink(mut control: Control<'static>) -> ! {
    loop {
        Delay.delay_ms(500).await;
        control.gpio_set(0, true).await;
        Delay.delay_ms(500).await;
        control.gpio_set(0, false).await;
    }
}