#![no_std]
#![no_main]

pub mod debug;

extern crate alloc;

use core::{
    arch::asm,
    borrow::{Borrow, BorrowMut},
    hint::unreachable_unchecked,
};

use alloc::vec::Vec;
use log::info;
use uefi::{
    prelude::*,
    proto::{
        loaded_image::LoadedImage,
        media::file::{File, FileAttribute, FileInfo, FileMode},
    },
    table::{
        boot::{AllocateType, EventType, MemoryMap, MemoryType, TimerTrigger, Tpl, PAGE_SIZE},
        runtime::Time,
    },
    CStr16,
};

use common::{
    paging::PageMap,
    registers::{CR0, CR3, CR4, IA32_EFER},
};

/* Note [The kernel's entrypoint]

`KERNEL_ENTRYPOINT` is in virtual address space. It's slightly offset to make room for a stack.
*/
const KERNEL_ENTRYPOINT: u64 = 0x1000;

#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init(&mut system_table).unwrap();

    uefi::println!("Booting...");

    {
        let image_base: u64 = {
            let loaded_image = system_table
                .boot_services()
                .open_protocol_exclusive::<LoadedImage>(image_handle)
                .unwrap();
            let (image_base, _) = loaded_image.info();
            image_base as u64
        };

        info!("image base: {:#x})", image_base);
    }

    if check_boot_interruption(&mut system_table) {
        browse_memory_map(&mut system_table);
    }

    let (kernel_addr, kernel_num_pages) =
        match load_kernel(image_handle, &mut system_table, cstr16!("kernel.bin")) {
            Err(err) => {
                return err;
            }
            Ok(value) => value,
        };

    // TODO: exit boot services and allocate pages from UEFI's memory map.

    let mut allocate_pages = |n| {
        system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, n)
            .unwrap()
    };

    let mut page_map = PageMap::new(&mut allocate_pages, PAGE_SIZE);

    map_stack(&mut allocate_pages, &mut page_map);

    map_kernel(
        &mut allocate_pages,
        &mut page_map,
        kernel_addr,
        kernel_num_pages,
    );

    map_switch_to_kernel(&mut allocate_pages, &mut page_map);

    // TODO: map the rest of available memory?
    info!("pml4 address: {:#x}", page_map.address());
    info!("total memory mapped: {}B", page_map.size());

    page_map.debug(
        &mut |index, pml4e| {
            info!("pml4e {}: {:#x}", index, pml4e.value());
        },
        &mut |index, pdpte| {
            info!("  pdpte {}: {:#x}", index, pdpte.value());
        },
        &mut |index, pde| {
            info!("    pde {}: {:#x}", index, pde.value());
        },
        &mut |index, virtual_address, pte| {
            info!(
                "      pte {} ({:#x}): {:#x}",
                index,
                virtual_address,
                pte.value()
            );
        },
    );

    /* 4-level paging requires:

    * CR0.PG = 1
    * CR4.PAE = 1
    * IA32_EFER.LME = 1
    * CR4.LA57 = 0

    Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Section 4.5
    */
    let cr0 = CR0::read();
    let cr4 = CR4::read();
    let ia32_efer = IA32_EFER::read();
    assert!(
        cr0.pg() && cr4.pae() && ia32_efer.lme() && !cr4.la57(),
        "4-level paging isn't enabled"
    );

    unsafe { switch_to_kernel(page_map) }
}

unsafe fn switch_to_kernel(page_map: PageMap) -> ! {
    let mut cr3 = CR3::read();

    cr3.set_address(page_map.address());

    /* Note [Disabling interrupts before writing to CR3]

    When interrupts aren't disabled, I get a page fault immediately after writing to
    CR3. The CPU tries to access the IDT that UEFI installed, but the IDT's address
    is no longer mapped.

    TODO: allocate and map my own IDT, then re-enable interrupts.
     */
    asm!("cli");

    cr3.write();

    asm!(
        "mov rbp, {0}",
        "mov rsp, {0}",
        "mov rdi, {1}",
        "jmp {0}",
        // See Note [The kernel's virtual address]
        in(reg) KERNEL_ENTRYPOINT,
        in(reg) PAGE_SIZE
    );

    unreachable_unchecked()
}

fn load_kernel(
    image_handle: Handle,
    system_table: &mut SystemTable<Boot>,
    kernel_file_name: &CStr16,
) -> Result<(u64, usize), uefi::Status> {
    let mut kernel_file = {
        let mut fs = system_table
            .boot_services()
            .get_image_file_system(image_handle)
            .unwrap();

        let mut root = fs.open_volume().unwrap();

        match root.open(kernel_file_name, FileMode::Read, FileAttribute::empty()) {
            Ok(file) => file.into_regular_file().unwrap(),
            Err(err) => match err.status() {
                Status::NOT_FOUND => {
                    uefi::println!("error: {} not found", kernel_file_name);
                    return Err(Status::ABORTED);
                }
                _ => {
                    uefi::println!("error: failed to open {}: {}", kernel_file_name, err);
                    return Err(Status::ABORTED);
                }
            },
        }
    };

    let kernel_size = {
        /*
        The definition of [FileInfo](https://docs.rs/uefi/latest/uefi/proto/media/file/struct.FileInfo.html) is:

        ```rust
        #[repr(C)]
        pub struct FileInfo {
            size: u64,
            file_size: u64,
            physical_size: u64,
            create_time: Time,
            last_access_time: Time,
            modification_time: Time,
            attribute: FileAttribute,
            file_name: [Char16],
        }
        ```

        It doesn't have `Sized` because of the `[Char16]` at the end.
        Its minimum size (if the file name was 0 characters, and ignoring alignment padding) would be the sum of the
        sizes of all other fields.
        */
        let min_fileinfo_size = core::mem::size_of::<u64>() * 3
            + core::mem::size_of::<Time>() * 3
            + core::mem::size_of::<FileAttribute>();

        // Since the file name will be more than zero characters, the grow loop will be triggered at least once.
        // It's less efficient, but I'm okay paying that cost to test the grow loop.
        let kernel_file_info = alloc_growing_ref(system_table, min_fileinfo_size, |storage| {
            kernel_file.get_info::<FileInfo>(storage)
        })
        .unwrap();

        kernel_file_info.as_ref().file_size() as usize
    };

    let kernel_pages = (kernel_size + PAGE_SIZE - 1) / PAGE_SIZE;
    let kernel_addr: u64 = system_table
        .boot_services()
        .allocate_pages(
            AllocateType::AnyPages,
            MemoryType::LOADER_DATA,
            kernel_pages,
        )
        .unwrap();

    info!("kernel address: {:#x}", kernel_addr);
    info!(
        "allocated {} pages for {}B kernel",
        kernel_pages, kernel_size
    );

    let kernel_buffer: &mut [u8] =
        unsafe { core::slice::from_raw_parts_mut(kernel_addr as *mut u8, kernel_size) };
    kernel_file.read(kernel_buffer).unwrap();
    kernel_file.close();
    info!("finished reading kernel into memory");

    Ok((kernel_addr, kernel_pages))
}

fn map_stack(allocate_pages: &mut dyn FnMut(usize) -> u64, page_map: &mut PageMap) {
    // Assumes that the stack precedes the kernel in virtual address space.
    let stack_physical_address = allocate_pages((KERNEL_ENTRYPOINT / (PAGE_SIZE as u64)) as usize);
    page_map.set_writable(allocate_pages, 0x0, stack_physical_address);
}

fn map_kernel(
    allocate_pages: &mut dyn FnMut(usize) -> u64,
    page_map: &mut PageMap,
    kernel_physical_addr: u64,
    kernel_num_pages: usize,
) {
    let mut next_virtual_page_address = KERNEL_ENTRYPOINT;
    let mut next_physical_page_address = kernel_physical_addr;
    for _page in 0..kernel_num_pages {
        page_map.set(
            allocate_pages,
            next_virtual_page_address,
            next_physical_page_address,
        );

        next_virtual_page_address += PAGE_SIZE as u64;
        next_physical_page_address += PAGE_SIZE as u64;
    }
    info!("finished setting up page map for kernel");
}

fn map_switch_to_kernel(allocate_pages: &mut dyn FnMut(usize) -> u64, page_map: &mut PageMap) {
    let switch_to_kernel_addr: u64 = unsafe {
        let addr: u64;
        asm!("2: lea {0}, {1}", out(reg) addr, sym switch_to_kernel);
        addr
    };

    /* The address of the 4KiB aligned page in which the `switch_to_kernel` function resides.

    If paging is enabled, then UEFI virtual memory is identity-mapped (<https://uefi.org/specs/UEFI/2.9_A/02_Overview.html?highlight=identity#ia-32-platforms>).
    Therefore this address is also the phsical address of the page.

    As long as the `switch_to_kernel` function stays under 4KiB in length, this page is
    the only one that needs to be identity-mapped into the kernel's virtual address space.
    Ideally, the kernel would remove the mapping after the switch.

    I think the only constraint on `switch_to_kernel`'s location is that it doesn't overlap
    with the kernel's virtual address. If we work at the granularity of a page: `switch_to_kernel`'s virtual address isn't in the first page of the kernel.

    If the kernel lives at 0x0 then 0x0-0xfff would be out of bounds, while everything else
    is in.
    */
    let switch_to_kernel_page_addr: u64 = switch_to_kernel_addr & !0xfff;
    assert!(
        switch_to_kernel_page_addr > KERNEL_ENTRYPOINT + 0xfff,
        "switch_to_kernel overlaps with start of kernel"
    );

    info!(
        "switch_to_kernel page address: {:#x}",
        switch_to_kernel_page_addr
    );

    /* The page in which `switch_to_kernel` needs to be identity-mapped because that
    function is going to set `page_map` as the active virtual memory map. After this
    happens, instruction fetches need to return the remaining instructions of
    `switch_to_kernel`. When this region of code isn't mapped, the instruction fetches
    will cause page faults.
    */
    page_map.set(
        allocate_pages,
        switch_to_kernel_page_addr,
        switch_to_kernel_page_addr,
    );
    info!("finished setting up page map for context switch");
}

struct PooledRef<'a, T: ?Sized> {
    system_table: &'a mut SystemTable<Boot>,
    address: *mut u8,
    typed_reference: &'a mut T,
}

impl<'a, T: ?Sized> Drop for PooledRef<'a, T> {
    fn drop(&mut self) {
        unsafe {
            let _ = self.system_table.boot_services().free_pool(self.address);
        };
    }
}

impl<'a, T: ?Sized> AsRef<T> for PooledRef<'a, T> {
    fn as_ref(&self) -> &T {
        self.typed_reference
    }
}

impl<'a, T: ?Sized> Borrow<T> for PooledRef<'a, T> {
    fn borrow(&self) -> &T {
        self.typed_reference
    }
}

impl<'a, T: ?Sized> BorrowMut<T> for PooledRef<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        self.typed_reference
    }
}

fn alloc_growing_ref<'a, T: ?Sized, E: core::fmt::Debug>(
    system_table: &'a mut SystemTable<Boot>,
    initial_size: usize,
    mut f: impl FnMut(&mut [u8]) -> uefi::Result<&mut T, E>,
) -> uefi::Result<PooledRef<'a, T>, E> {
    let mut storage_size = initial_size;
    let mut storage_addr: *mut u8;

    let mut buffer: &mut [u8];
    let buffer_typed: &mut T;

    let boot_services = system_table.boot_services();
    loop {
        storage_addr = boot_services
            .allocate_pool(MemoryType::BOOT_SERVICES_DATA, storage_size)
            .unwrap();
        buffer = unsafe { core::slice::from_raw_parts_mut(storage_addr, storage_size) };

        info!("trying get_info with buffer size {}", storage_size);
        match f(buffer) {
            Ok(value) => {
                info!("alloc_growing_ref succeeded");
                buffer_typed = value;
                break;
            }
            Err(err) => {
                info!("alloc_growing_ref failed (buffer too small)");
                unsafe { boot_services.free_pool(storage_addr).unwrap() };

                match err.status() {
                    Status::BUFFER_TOO_SMALL => {
                        storage_size += 100;
                        continue;
                    }
                    _ => {
                        return Err(err);
                    }
                }
            }
        }
    }

    Ok(PooledRef {
        system_table,
        address: storage_addr,
        typed_reference: buffer_typed,
    })
}
struct Pooled<'a, T> {
    system_table: &'a mut SystemTable<Boot>,
    buffer: *mut u8,
    value: T,
}

impl<'a, T> Drop for Pooled<'a, T> {
    fn drop(&mut self) {
        unsafe {
            let _ = self.system_table.boot_services().free_pool(self.buffer);
        };
    }
}

impl<'a, T> AsRef<T> for Pooled<'a, T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<'a, T> Borrow<T> for Pooled<'a, T> {
    fn borrow(&self) -> &T {
        &self.value
    }
}

impl<'a, T> BorrowMut<T> for Pooled<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

fn alloc_growing<'a, T, E: core::fmt::Debug>(
    system_table: &'a mut SystemTable<Boot>,
    initial_size: usize,
    mut f: impl FnMut(&mut [u8]) -> uefi::Result<T, E>,
) -> uefi::Result<Pooled<'a, T>, E> {
    let mut storage_size = initial_size;
    let mut storage_addr: *mut u8;

    let mut buffer: &mut [u8];
    let value: T;

    let boot_services = system_table.boot_services();
    loop {
        storage_addr = boot_services
            .allocate_pool(MemoryType::BOOT_SERVICES_DATA, storage_size)
            .unwrap();
        buffer = unsafe { core::slice::from_raw_parts_mut(storage_addr, storage_size) };

        info!("trying get_info with buffer size {}", storage_size);
        match f(buffer) {
            Ok(new_value) => {
                info!("alloc_growing succeeded");
                value = new_value;
                break;
            }
            Err(err) => {
                info!("alloc_growing failed (buffer too small)");
                unsafe { boot_services.free_pool(storage_addr).unwrap() };

                match err.status() {
                    Status::BUFFER_TOO_SMALL => {
                        storage_size += 100;
                        continue;
                    }
                    _ => {
                        return Err(err);
                    }
                }
            }
        }
    }

    Ok(Pooled {
        system_table,
        buffer: storage_addr,
        value,
    })
}

fn check_boot_interruption(system_table: &mut SystemTable<Boot>) -> bool {
    uefi::println!("Press any key to interrupt boot.");
    let boot_delay_seconds = 2;

    // <https://github.com/tianocore-docs/edk2-UefiDriverWritersGuide/blob/master/5_uefi_services/51_services_that_uefi_drivers_commonly_use/516_settimer.md>
    let timer_event = unsafe {
        system_table
            .boot_services()
            .create_event(EventType::TIMER, Tpl::CALLBACK, None, None)
    }
    .unwrap();

    system_table
        .boot_services()
        .set_timer(
            &timer_event,
            TimerTrigger::Relative(boot_delay_seconds * 1_000_000_000 / 100),
        )
        .unwrap();

    let wait_for_key_event = system_table.stdin().wait_for_key_event().unwrap();

    let index = system_table
        .boot_services()
        .wait_for_event(&mut [wait_for_key_event, timer_event])
        .unwrap();

    match index {
        0 => {
            system_table.stdin().read_key().unwrap();
            true
        }
        1 => false,
        _ => panic!("unexpected index returned from wait_for_event: {}", index),
    }
}

fn with_memory_map<T>(
    system_table: &mut SystemTable<Boot>,
    f: impl FnOnce(&mut SystemTable<Boot>, &mut MemoryMap) -> T,
) -> T {
    let boot_services = system_table.boot_services();

    let memory_map_sizes = boot_services.memory_map_size();
    let mut buffer: Vec<u8> = core::iter::repeat(0_u8)
        .take(memory_map_sizes.map_size)
        .collect();
    let mut memory_map: MemoryMap;

    loop {
        match boot_services.memory_map(&mut buffer) {
            Err(err) => match err.status() {
                Status::BUFFER_TOO_SMALL => {
                    buffer.extend(core::iter::repeat(0_u8).take(memory_map_sizes.entry_size));
                    continue;
                }
                _ => {
                    panic!("unexpected error: {}", err);
                }
            },
            Ok(new_memory_map) => {
                memory_map = new_memory_map;
                break;
            }
        }
    }

    f(system_table, &mut memory_map)
}

fn browse_memory_map(system_table: &mut SystemTable<Boot>) {
    with_memory_map(system_table, |system_table, memory_map| {
        memory_map.sort();

        let wait_for_key_event = system_table.stdin().wait_for_key_event().unwrap();
        let mut events = [wait_for_key_event];
        info!("found {} memory map entries", memory_map.entries().len());
        'outer: for (index, memory_map_entry) in memory_map.entries().enumerate() {
            'inner: loop {
                uefi::println!("print next entry? (y = yes, n = no/next, q = skip to end)");
                system_table
                    .boot_services()
                    .wait_for_event(&mut events)
                    .unwrap();
                match system_table.stdin().read_key().unwrap() {
                    Some(uefi::proto::console::text::Key::Printable(c)) => {
                        if c == 'y' {
                            break 'inner;
                        } else if c == 'n' {
                            continue 'outer;
                        } else if c == 'q' {
                            break 'outer;
                        } else {
                            continue 'inner;
                        }
                    }
                    _ => {
                        continue 'inner;
                    }
                }
            }

            uefi::println!("memory map entry {}", index);
            uefi::println!("  memory type: {:?}", memory_map_entry.ty);
            uefi::println!("  physical start: {:?}", memory_map_entry.phys_start);
            uefi::println!("  virtual start: {:?}", memory_map_entry.virt_start);
            uefi::println!("  page count: {:?}", memory_map_entry.page_count);
            uefi::println!("  attribute: {:?}", memory_map_entry.att);
        }
    })
}
