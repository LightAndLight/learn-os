#![no_main]
#![no_std]

use core::{
    arch::{asm, global_asm},
    hint::unreachable_unchecked,
};

use common::registers::CR3;

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

pub extern "sysv64" fn kernel(_page_size: usize) -> ! {
    let _cr3 = CR3::read();
    unsafe {
        asm!("2: mov rax, rax", "jmp 2b");
        unreachable_unchecked()
    }
}
