#![no_main]
#![no_std]

use core::{arch::asm, hint::unreachable_unchecked};

use common::registers::CR3;

#[no_mangle]
pub extern "sysv64" fn kernel(_page_size: usize) -> ! {
    let _cr3 = CR3::read();
    unsafe {
        asm!("2: mov rax, rax", "jmp 2b");
        unreachable_unchecked()
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
