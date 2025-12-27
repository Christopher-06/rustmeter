use esp_hal::gpio::Pin;
#[cfg(feature = "esp32")]
use esp_hal::{gpio::AnyPin, peripherals};

pub struct Config<'a> {
    /// UART peripheral to use for async printing
    pub(crate) uart_p: peripherals::UART0<'a>,
    /// Receive pin
    pub(crate) rx_pin: AnyPin<'a>,
    /// Transmit pin
    pub(crate) tx_pin: AnyPin<'a>,
    /// Baudrate for async printing
    pub(crate) baudrate: u32,
}

impl<'a> Config<'a> {
    pub const fn with_baudrate(mut self, baudrate: u32) -> Self {
        self.baudrate = baudrate;
        self
    }

    pub const fn with_pins(mut self, tx_pin: AnyPin<'a>, rx_pin: AnyPin<'a>) -> Self {
        self.tx_pin = tx_pin;
        self.rx_pin = rx_pin;
        self
    }

    pub const fn with_rx_pin(mut self, rx_pin: AnyPin<'a>) -> Self {
        self.rx_pin = rx_pin;
        self
    }

    pub const fn with_tx_pin(mut self, tx_pin: AnyPin<'a>) -> Self {
        self.tx_pin = tx_pin;
        self
    }

    pub fn new() -> Self {
        use esp_hal::peripherals::*;

        // Default pins based on selected ESP target (took from: https://github.com/esp-rs/esp-hal/blob/main/examples/async/embassy_serial/src/main.rs)
        cfg_if::cfg_if! {
            if #[cfg(feature = "esp32")] {
                let (tx_pin, rx_pin) = unsafe {(GPIO1::steal(),  GPIO3::steal())};
            } else if #[cfg(feature = "esp32c2")] {
                let (tx_pin, rx_pin) = unsafe {(GPIO20::steal(),  GPIO19::steal())};
            } else if #[cfg(feature = "esp32c3")] {
                let (tx_pin, rx_pin) = unsafe {(GPIO21::steal(),  GPIO20::steal())};
            } else if #[cfg(feature = "esp32c6")] {
                let (tx_pin, rx_pin) = unsafe {(GPIO16::steal(),  GPIO17::steal())};
            } else if #[cfg(feature = "esp32h2")] {
                let (tx_pin, rx_pin) = unsafe {(GPIO24::steal(),  GPIO23::steal())};
            } else if #[cfg(any(feature = "esp32s2", feature = "esp32s3"))] {
                let (tx_pin, rx_pin) = unsafe {(GPIO43::steal(),  GPIO44::steal())};
            }
        }

        Self {
            uart_p: unsafe { UART0::steal() },
            baudrate: 921_600,
            rx_pin: rx_pin.degrade(),
            tx_pin: tx_pin.degrade(),
        }
    }
}

impl<'a> Default for Config<'a> {
    fn default() -> Self {
        Self::new()
    }
}
