/*! Rust panic handling.

This module contains the [panic handler](https://doc.rust-lang.org/nomicon/panic-handler.html) definition for the kernel.

[`init_panic_logger`] enables serial output for panics.
*/
use core::{hint::unreachable_unchecked, ptr::addr_of_mut};

use crate::serial::PC16500D;

static mut PANIC_LOGGER: Option<PC16500D> = None;

/** Set the serial device to which panics should write.

# Safety

Not thread safe.
*/
pub unsafe fn init_panic_logger(serial_device: PC16500D) {
    PANIC_LOGGER = Some(serial_device)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
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

/* This function is only called from `panic`, so by default it gets inlined. That makes it
harder to see if the panic handler is causing code bloat due to stack unwinding.
*/
#[inline(never)]
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
