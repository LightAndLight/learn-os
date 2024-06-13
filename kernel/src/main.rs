#![no_main]
#![no_std]
#![feature(panic_info_message)]

pub mod io;
pub mod serial;

use core::{
    arch::{asm, global_asm},
    fmt::Write,
    hint::unreachable_unchecked,
    ptr::addr_of_mut,
};

use common::registers::CR3;
use io::IoPort;
use serial::PC16500D;

static mut PANIC_LOGGER: Option<PC16500D> = None;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    /* `Display` for integer types doesn't work in `no_std`. I think it tries to allocate.

    This method avoids allocation, at the cost of some "redundant" work.
    */
    fn write_u32(buffer: &mut dyn core::fmt::Write, value: u32) -> core::fmt::Result {
        // The largest power of 10 less than 2^32
        let mut divisor = 1_000_000_000;

        let mut n = value;
        let mut started = false;

        loop {
            let quotient = n / divisor;
            if quotient > 0 || started {
                let c = match quotient {
                    0 => '0',
                    1 => '1',
                    2 => '2',
                    3 => '3',
                    4 => '4',
                    5 => '5',
                    6 => '6',
                    7 => '7',
                    8 => '8',
                    9 => '9',
                    _ => unsafe { unreachable_unchecked() },
                };
                match buffer.write_char(c) {
                    Err(err) => {
                        return Err(err);
                    }
                    Ok(()) => {}
                }
                started = true;

                if divisor == 1 {
                    break;
                } else {
                    n = n % divisor;
                }
            }

            divisor /= 10;
        }

        Ok(())
    }

    fn write_location(
        buffer: &mut dyn core::fmt::Write,
        location: &core::panic::Location,
    ) -> core::fmt::Result {
        buffer.write_str(location.file())?;
        buffer.write_char(':')?;
        write_u32(buffer, location.line())?;
        buffer.write_char(':')?;
        write_u32(buffer, location.column())
    }

    fn write_panic_info(
        buffer: &mut dyn core::fmt::Write,
        info: &core::panic::PanicInfo,
    ) -> core::fmt::Result {
        buffer.write_str("panicked at ")?;
        match info.location() {
            None => buffer.write_str("(no location info)"),
            Some(location) => write_location(buffer, location),
        }?;
        buffer.write_char(':')?;
        if let Some(message) = info.message() {
            buffer.write_str("\n")?;
            buffer.write_fmt(*message)?;
        } else if let Some(payload) = info.payload().downcast_ref::<&'static str>() {
            buffer.write_str("\n")?;
            buffer.write_str(payload)?;
        }
        Ok(())
    }

    unsafe {
        match addr_of_mut!(PANIC_LOGGER).as_mut().and_then(Option::as_mut) {
            None => {}
            Some(serial_device) => {
                let _ = write_panic_info(serial_device, info);
            }
        }
    }

    loop {}
}

global_asm!(
    ".section .text.entrypoint",
    ".global _start",
    "_start:",
    "jmp {0}",
    sym kernel
);

pub extern "sysv64" fn kernel(_page_size: usize, serial_device_port: u16) -> ! {
    unsafe {
        PANIC_LOGGER = Some(PC16500D::new(IoPort(serial_device_port)));
    }

    let mut serial_device = unsafe { PC16500D::new(IoPort(serial_device_port)) };

    let _cr3 = CR3::read();

    writeln!(serial_device, "hello from kernel!").unwrap();

    assert!(false, "false is not true");

    unsafe {
        asm!("2: mov rax, rax", "jmp 2b");
        unreachable_unchecked()
    }
}
