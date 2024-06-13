#![no_main]
#![no_std]

pub mod io;
pub mod serial;

use core::{
    arch::{asm, global_asm},
    fmt::Write,
    hint::unreachable_unchecked,
};

use common::registers::CR3;
use io::IoPort;
use serial::PC16500D;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
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
    let mut serial_device = unsafe { PC16500D::new(IoPort(serial_device_port)) };

    let _cr3 = CR3::read();

    unsafe {
        serial_device.set_loopback(true);
        serial_device.write_u8(0xae);
        let value = serial_device.read_u8();
        if value != 0xae {
            asm!("2: mov rbx, rbx", "jmp 2b");
        }
        serial_device.set_loopback(false);
    }

    writeln!(serial_device, "hello from kernel!").unwrap();

    unsafe {
        asm!("2: mov rax, rax", "jmp 2b");
        unreachable_unchecked()
    }
}
