#![no_main]
#![no_std]
#![feature(panic_info_message)]

pub mod io;
pub mod panic;
pub mod serial;

use core::{
    arch::{asm, global_asm},
    fmt::Write,
    hint::unreachable_unchecked,
};

use common::registers::CR3;
use io::IoPort;
use panic::init_panic_logger;
use serial::PC16500D;

global_asm!(
    ".section .text.entrypoint",
    ".global _start",
    "_start:",
    "jmp {0}",
    sym kernel
);

pub extern "sysv64" fn kernel(_page_size: usize, serial_device_port: u16) -> ! {
    unsafe {
        init_panic_logger(PC16500D::new(IoPort(serial_device_port)));
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
