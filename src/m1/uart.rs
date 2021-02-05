use register::mmio::ReadWrite;

#[allow(non_snake_case)]
#[repr(C)]
pub struct UARTRegister {
    ULCON: ReadWrite<u32>,
    UCON: ReadWrite<u32>,
    UFCON: ReadWrite<u32>,
    UMCON: ReadWrite<u32>,
    UTRSTAT: ReadWrite<u32>,
    UERSTAT: ReadWrite<u32>,
    UFSTAT: ReadWrite<u32>,
    UMSTAT: ReadWrite<u32>,
    UTXH: ReadWrite<u32>,
    URXH: ReadWrite<u32>,
    UBRDIV: ReadWrite<u32>,
}

unsafe impl core::marker::Sync for UART {}

pub struct UART {
    pub register_base: *const UARTRegister,
}

pub const UART_CLOCK: u32 = 24000000;

pub const UTRSTAT_RXDR: u32 = 1 << 0;
pub const UTRSTAT_TXFE: u32 = 1 << 1;
pub const UTRSTAT_TXE: u32 = 1 << 2;
pub const UTRSTAT_TIMEOUT: u32 = 1 << 3;

impl UART {
    pub const INSTANCE: Self = UART {
        register_base: 0x2352_00000 as *const UARTRegister,
    };

    pub fn init(&self, baud_rate: u32) {
        self.set_baudrate(baud_rate);
    }

    pub fn set_baudrate(&self, baud_rate: u32) {
        self.wait_status(UTRSTAT_TXE);

        let ubr_div = unsafe { &((*self.register_base).UBRDIV) };

        ubr_div.set(((UART_CLOCK / baud_rate + 7) / 16) - 1);
    }

    pub fn wait_status(&self, val: u32) {
        let utr_stat = unsafe { &((*self.register_base).UTRSTAT) };

        while (utr_stat.get() & val) == 0 {}
    }

    pub fn wait_transmit(&self) {
        self.wait_status(UTRSTAT_TXFE);
    }

    pub fn wait_receive(&self) {
        self.wait_status(UTRSTAT_RXDR);
    }

    pub fn put_byte(&self, val: u8) {
        self.wait_transmit();

        let transmit_reg = unsafe { &((*self.register_base).UTXH) };

        transmit_reg.set(u32::from(val));
    }

    pub fn write_data(&self, data: &[u8]) {
        for val in data {
            self.put_byte(*val);
        }
    }

    pub fn get_byte(&self) -> u8 {
        self.wait_receive();

        let receive_reg = unsafe { &((*self.register_base).URXH) };
        receive_reg.get() as u8
    }

    pub fn put_u32(&self, d: u32) {
        let mut digits: [u8; 10] = [0x0; 10];
        let mut d = d;

        for i in digits.iter_mut() {
            *i = ((d % 10) + 0x30) as u8;

            d /= 10;

            if d == 0 {
                break;
            }
        }

        for c in digits.iter().rev() {
            self.put_byte(*c);
        }
    }

    pub fn put_u64(&self, d: u64) {
        let mut digits: [u8; 20] = [0x0; 20];
        let mut d = d;

        for i in digits.iter_mut() {
            *i = ((d % 10) + 0x30) as u8;

            d /= 10;

            if d == 0 {
                break;
            }
        }

        for c in digits.iter().rev() {
            self.put_byte(*c);
        }
    }
}

impl core::fmt::Write for UART {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        self.write_data(s.as_bytes());
        Ok(())
    }
}
