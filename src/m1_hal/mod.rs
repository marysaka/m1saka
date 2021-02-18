use crate::m1::uart::UART;
use embedded_hal::serial::{Read, Write};

impl Write<u8> for UART {
    // No error possible
    type Error = ();

    fn write(&mut self, data: u8) -> nb::Result<(), Self::Error> {
        self.put_byte(data);
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.wait_transmit();
        Ok(())
    }
}

impl Read<u8> for UART {
    // No error possible
    type Error = ();

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        Ok(self.get_byte())
    }
}

impl Write<u16> for UART {
    // No error possible
    type Error = ();

    fn write(&mut self, data: u16) -> nb::Result<(), Self::Error> {
        self.write_data(&u16::to_le_bytes(data)[..]);
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.wait_transmit();
        Ok(())
    }
}

impl Write<u32> for UART {
    // No error possible
    type Error = ();

    fn write(&mut self, data: u32) -> nb::Result<(), Self::Error> {
        self.write_data(&u32::to_le_bytes(data)[..]);
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.wait_transmit();
        Ok(())
    }
}

impl Write<u64> for UART {
    // No error possible
    type Error = ();

    fn write(&mut self, data: u64) -> nb::Result<(), Self::Error> {
        self.write_data(&u64::to_le_bytes(data)[..]);
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.wait_transmit();
        Ok(())
    }
}

impl Write<&[u8]> for UART {
    // No error possible
    type Error = ();

    fn write(&mut self, data: &[u8]) -> nb::Result<(), Self::Error> {
        self.write_data(data);
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.wait_transmit();
        Ok(())
    }
}
