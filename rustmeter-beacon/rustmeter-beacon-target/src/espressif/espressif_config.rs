use esp_hal::{
    gpio::{AnyPin, Pin},
    peripherals::{self, UART0},
    time::Rate,
    uart::{self, Uart},
};

use crate::espressif::printing::PrinterRoute;

pub trait ConfigPrinterBuild {
    fn clone_unchecked(&self) -> Self;
    fn build_printer_route(self) -> Result<PrinterRoute, uart::ConfigError>;
}

pub struct UartPrintSelection {
    uart_p: UART0<'static>,
    tx_pin: AnyPin<'static>,
    rx_pin: AnyPin<'static>,
    baudrate: u32,
}

impl UartPrintSelection {
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

impl ConfigPrinterBuild for UartPrintSelection {
    fn clone_unchecked(&self) -> Self {
        unsafe {
            Self {
                uart_p: self.uart_p.clone_unchecked(),
                tx_pin: self.tx_pin.clone_unchecked(),
                rx_pin: self.rx_pin.clone_unchecked(),
                baudrate: self.baudrate,
            }
        }
    }

    /// Steal the UART peripheral and build the printer route
    fn build_printer_route(self) -> Result<PrinterRoute, uart::ConfigError> {
        let uart = unsafe {
            Uart::new(
                self.uart_p.clone_unchecked(),
                uart::Config::default().with_baudrate(self.baudrate),
            )?
            .with_rx(self.rx_pin.clone_unchecked())
            .with_tx(self.tx_pin.clone_unchecked())
            .into_async()
        };

        Ok(PrinterRoute::Uart(uart))
    }
}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
pub struct SerialJtagPrintSelection {
    periph: peripherals::USB_DEVICE<'static>,
}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
impl SerialJtagPrintSelection {
    pub fn new() -> Self {
        Self {
            periph: unsafe { peripherals::USB_DEVICE::steal() },
        }
    }
}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
impl ConfigPrinterBuild for SerialJtagPrintSelection {
    /// Steal the USB_DEVICE peripheral and build the printer route
    fn build_printer_route(self) -> Result<PrinterRoute, uart::ConfigError> {
        let jtag = esp_hal::usb_serial_jtag::UsbSerialJtag::new(self.periph).into_async();

        Ok(PrinterRoute::SerialJtag(jtag))
    }

    fn clone_unchecked(&self) -> Self {
        unsafe {
            Self {
                periph: self.periph.clone_unchecked(),
            }
        }
    }
}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
pub struct AutoPrintSelection {
    serial_jtag: SerialJtagPrintSelection,
    uart: UartPrintSelection,
}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
impl AutoPrintSelection {
    pub fn new() -> Self {
        Self {
            serial_jtag: SerialJtagPrintSelection::new(),
            uart: UartPrintSelection::new(),
        }
    }

    /// Decide automatically which printer to use based on available peripherals. Code taken from esp-hal/esp-println:
    /// https://github.com/esp-rs/esp-hal/blob/main/esp-println/src/lib.rs
    fn use_jtag() -> bool {
        // Decide if serial-jtag is used by checking SOF interrupt flag.
        // SOF packet is sent by the HOST every 1ms on a full speed bus.
        // Between two consecutive ticks, there will be at least 1ms (selectable tick
        // rate range is 1 - 1000Hz).
        // We don't reset the flag - if it was ever connected we assume serial-jtag is
        // used
        #[cfg(feature = "esp32c3")]
        const USB_DEVICE_INT_RAW: *const u32 = 0x60043008 as *const u32;
        #[cfg(feature = "esp32c6")]
        const USB_DEVICE_INT_RAW: *const u32 = 0x6000f008 as *const u32;
        #[cfg(feature = "esp32h2")]
        const USB_DEVICE_INT_RAW: *const u32 = 0x6000f008 as *const u32;
        #[cfg(feature = "esp32s3")]
        const USB_DEVICE_INT_RAW: *const u32 = 0x60038000 as *const u32;

        const SOF_INT_MASK: u32 = 0b10;

        unsafe { (USB_DEVICE_INT_RAW.read_volatile() & SOF_INT_MASK) != 0 }
    }
}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
impl ConfigPrinterBuild for AutoPrintSelection {
    /// Automatically decide which printer to build based on available peripherals
    fn build_printer_route(self) -> Result<PrinterRoute, uart::ConfigError> {
        if Self::use_jtag() {
            self.serial_jtag.build_printer_route()
        } else {
            self.uart.build_printer_route()
        }
    }

    fn clone_unchecked(&self) -> Self {
        Self {
            serial_jtag: self.serial_jtag.clone_unchecked(),
            uart: self.uart.clone_unchecked(),
        }
    }
}

pub struct Config<Printer: ConfigPrinterBuild> {
    printer: Printer,
    pub(crate) cpu_freq: Rate,
}

impl<Printer: ConfigPrinterBuild> Config<Printer> {
    /// Use UART for printing
    pub fn with_uart_printer(self) -> Config<UartPrintSelection> {
        Config {
            printer: UartPrintSelection::new(),
            cpu_freq: self.cpu_freq,
        }
    }

    #[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
    /// Use Serial JTAG for printing
    pub fn with_serial_jtag_printer(self) -> Config<SerialJtagPrintSelection> {
        Config {
            printer: SerialJtagPrintSelection::new(),
            cpu_freq: self.cpu_freq,
        }
    }

    #[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
    /// Automatically select the printer based on available peripherals
    pub fn with_auto_printer(self) -> Config<AutoPrintSelection> {
        Config {
            printer: AutoPrintSelection::new(),
            cpu_freq: self.cpu_freq,
        }
    }

    /// Set the CPU frequency for timing calculations
    pub fn with_cpu_freq(mut self, cpu_freq: Rate) -> Self {
        self.cpu_freq = cpu_freq;
        self
    }
}

impl Config<UartPrintSelection> {
    /// Create a new tracing configuration with the specified CPU frequency, using UART for printing.
    #[cfg(not(any(
        feature = "esp32s3",
        feature = "esp32c3",
        feature = "esp32c6",
        feature = "esp32h2"
    )))]
    pub fn new(cpu_rate: Rate) -> Config<UartPrintSelection> {
        Config {
            printer: UartPrintSelection::new(),
            cpu_freq: cpu_rate,
        }
    }

    /// Set the UART baudrate. Must also be set in rustmeter-cli if different than default 921600
    pub fn with_baudrate(mut self, baudrate: u32) -> Self {
        self.printer.baudrate = baudrate;
        self
    }

    /// Set the UART TX and RX pins
    pub fn with_pins(mut self, tx_pin: AnyPin<'static>, rx_pin: AnyPin<'static>) -> Self {
        self.printer.tx_pin = tx_pin;
        self.printer.rx_pin = rx_pin;
        self
    }

    /// Set the UART RX pin
    pub fn with_rx_pin(mut self, rx_pin: AnyPin<'static>) -> Self {
        self.printer.rx_pin = rx_pin;
        self
    }

    /// Set the UART TX pin
    pub fn with_tx_pin(mut self, tx_pin: AnyPin<'static>) -> Self {
        self.printer.tx_pin = tx_pin;
        self
    }
}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
impl Config<SerialJtagPrintSelection> {}

#[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
impl Config<AutoPrintSelection> {
    /// Create a new tracing configuration with the specified CPU frequency. It tries to automatically
    /// select the best printer available on the target device (JTAG if connected, otherwise UART).
    pub fn new(cpu_rate: Rate) -> Config<AutoPrintSelection> {
        Config {
            printer: AutoPrintSelection::new(),
            cpu_freq: cpu_rate,
        }
    }

    /// Set the UART baudrate when UART printing was selected automatically
    pub fn with_uart_baudrate(mut self, baudrate: u32) -> Self {
        self.printer.uart.baudrate = baudrate;
        self
    }

    /// Set the UART TX and RX pins when UART printing was selected automatically
    pub fn with_uart_pins(mut self, tx_pin: AnyPin<'static>, rx_pin: AnyPin<'static>) -> Self {
        self.printer.uart.tx_pin = tx_pin;
        self.printer.uart.rx_pin = rx_pin;
        self
    }

    /// Set the UART RX pin when UART printing was selected automatically
    pub fn with_uart_rx_pin(mut self, rx_pin: AnyPin<'static>) -> Self {
        self.printer.uart.rx_pin = rx_pin;
        self
    }

    /// Set the UART TX pin when UART printing was selected automatically
    pub fn with_uart_tx_pin(mut self, tx_pin: AnyPin<'static>) -> Self {
        self.printer.uart.tx_pin = tx_pin;
        self
    }
}

impl<Printer: ConfigPrinterBuild> Config<Printer> {
    /// Build the printer route based on the selected configuration
    pub fn build_printer_route(&self) -> Result<PrinterRoute, uart::ConfigError> {
        self.printer.clone_unchecked().build_printer_route()
    }
}
