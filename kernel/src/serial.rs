use crate::io::IoPort;

pub struct PC16500D {
    io_base: IoPort,
}

impl PC16500D {
    /** Initialise the serial device.

    # Safety

    The underlying I/O port must be mapped to a PC16500D serial device.
    */
    pub unsafe fn new(io_base: IoPort) -> Self {
        // io_base.add(1).outb(0x0);
        // io_base.add(2).outb(0x0);
        // io_base.add(4).outb(0xf);
        Self { io_base }
    }

    /** Read the line status register.

    # Safety

    The underlying I/O port must be mapped to a PC16500D serial device.
    */
    pub unsafe fn line_status(&mut self) -> u8 {
        self.io_base.add(5).read_u8()
    }

    /** Transmit a byte using the serial device.

    # Safety

    The underlying I/O port must be mapped to a PC16500D serial device.
    */
    pub unsafe fn write_u8(&mut self, value: u8) {
        while self.line_status() & 0x20 == 0 {}
        self.io_base.write_u8(value)
    }

    /** Receive a byte using the serial device.

    # Safety

    The underlying I/O port must be mapped to a PC16500D serial device.
    */
    pub unsafe fn read_u8(&mut self) -> u8 {
        while self.line_status() & 0x1 == 0 {}
        self.io_base.read_u8()
    }

    /** Put the serial device into loopback mode.

    Transmitted data will remain on the device and be immediately available for reading.

    # Safety

    The underlying I/O port must be mapped to a PC16500D serial device.
    */
    pub unsafe fn set_loopback(&mut self, value: bool) {
        let mut port = self.io_base.add(4);
        let modem_control = port.read_u8();
        let mask = 0x10;
        if value {
            port.write_u8(modem_control | mask);
        } else {
            port.write_u8(modem_control & !mask);
        }
    }
}

impl core::fmt::Write for PC16500D {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut buffer = [0_u8, 0, 0, 0];
        for c in s.chars() {
            c.encode_utf8(&mut buffer);
            for i in 0..c.len_utf8() {
                unsafe { self.write_u8(buffer[i]) }
            }
        }
        Ok(())
    }
}
