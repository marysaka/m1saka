use crate::m1::uart::UART;
use core::fmt::Write;
use log::{Level, Metadata, Record};
use log::{LevelFilter, SetLoggerError};

struct UARTLogger {
    level: Level,
}

impl UARTLogger {
    fn configure(&mut self, baud_rate: u32) {
        // TODO
        let mut uart = &UART::INSTANCE;

        uart.init(baud_rate);
    }
}

impl log::Log for UARTLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.level >= metadata.level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut uart = UART::INSTANCE;
            writeln!(&mut uart, "{} - {}\r", record.level(), record.args()).ok();
        }
    }

    fn flush(&self) {}
}

static mut LOGGER: UARTLogger = UARTLogger { level: Level::Info };

pub fn init(baud_rate: u32) -> Result<(), SetLoggerError> {
    unsafe {
        LOGGER.configure(baud_rate);

        log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Trace))
    }
}
