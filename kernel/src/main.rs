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

use common::{
    paging::{self, PageMap},
    registers::CR3,
};
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

/** The kernel's Rust entrypoint.

# Arguments

* `page_size` - The system's page size, according to the bootloader.

* `switch_to_kernel_page_addr` - Virtual address of the page allocated for the page table "pivot" function.

  A function that changes the page table needs to have the same virtual memory address
  before and after the switch. The bootloader sets up the kernel's page table, so it has
  code for this. The bootloader code that switches to the kernel's page table is also
  mapped in the kernel's page table.

  Once running, the kernel should unmap that page so that it can have the full virtual
  address space to itself. Therefore it needs to know the virtual address of the page
  that it will unmap.

* `serial_device_port` - I/O port for a PC16500D serial device.

  The bootloader uses UEFI's PCI protocols to discover a serial device so that I don't
  have to reimplement PCI handling in the kernel (for now).
*/
pub extern "sysv64" fn kernel(
    _page_size: usize,
    _switch_to_kernel_page_addr: u64,
    serial_device_port: u16,
) -> ! {
    /* Note [Kernel entrypoint arguments]

    In short, these arguments need to be passed in registers.

    The number of arguments to this function are likely to increase over time, so it would
    be prudent to put them into a struct. This is harder than it seems, because the struct's
    fields would need to be passed in registers.

    This function is called from the bootloader, which has its own stack. If this function
    is given a struct that's passed via the stack, then the struct will live in a stack frame
    on the bootloader's stack. But before this function is called `rbp` and `rsp` are updated
    to point to the kernel's stack. This function will look for the struct in its own stack
    frame and fail to find it. Thus registers are the only consistent way to pass data across
    the bootloader-kernel boundary.
    */

    unsafe {
        init_panic_logger(PC16500D::new(IoPort(serial_device_port)));
    }

    let mut serial_device = unsafe { PC16500D::new(IoPort(serial_device_port)) };

    let _page_map = PageMap::from_cr3();

    writeln!(serial_device, "hello from kernel!").unwrap();

    assert!(false, "false is not true");

    unsafe {
        asm!("2: mov rax, rax", "jmp 2b");
        unreachable_unchecked()
    }
}
