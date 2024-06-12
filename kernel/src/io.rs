use core::arch::asm;

pub struct IoPort(pub u16);

impl IoPort {
    /// Create a new [`IoPort`] relative to an existing one.
    pub fn add(&self, offset: u16) -> Self {
        Self(self.0 + offset)
    }

    /** Read a byte from an I/O port.

    # Safety

    The I/O port must be valid.
    */
    pub unsafe fn read_u8(&mut self) -> u8 {
        let value: u8;
        asm!("in al, dx", out("al") value, in("dx") self.0);
        value
    }

    /** Write a byte to an I/O port.

    # Safety

    The I/O port must be valid.
    */
    pub unsafe fn write_u8(&mut self, value: u8) {
        asm!("out dx, al", in("dx") self.0, in("al") value)
    }
}
