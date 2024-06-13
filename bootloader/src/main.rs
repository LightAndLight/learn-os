#![no_std]
#![no_main]

pub mod debug;

extern crate alloc;

use core::{
    arch::asm,
    borrow::{Borrow, BorrowMut},
    hint::unreachable_unchecked,
    ptr::NonNull,
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
        boot::{
            AllocateType, EventType, MemoryMap, MemoryType, OpenProtocolAttributes,
            OpenProtocolParams, TimerTrigger, Tpl, PAGE_SIZE,
        },
        runtime::Time,
    },
    CStr16,
};

use common::{
    exe::v0,
    paging::{PageMap, PageMapFlags},
    registers::{CR0, CR3, CR4, IA32_EFER},
};
use uefi_pci::{PciConfigurationAddress, PciRootBridgeIo};

/* Note [The kernel's entrypoint]

The kernel runs in its own virtual address space. The kernel code starts at `KERNEL_ENTRYPOINT`, and
everything between 0x0 and `KERNEL_ENTRYPOINT` is the kernel's stack.
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

        info!("image base: {:#x}", image_base);
    }

    if check_boot_interruption(&mut system_table) {
        browse_memory_map(&mut system_table);
    }

    let kernel_info = match load_kernel(image_handle, &mut system_table, cstr16!("kernel.bin")) {
        Err(err) => {
            return err;
        }
        Ok(value) => value,
    };

    let mut allocate_pages = |n| {
        system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, n)
            .unwrap()
    };

    let mut page_map = PageMap::new(&mut allocate_pages, PAGE_SIZE);

    map_stack(&mut allocate_pages, &mut page_map);

    map_kernel(&mut allocate_pages, &mut page_map, &kernel_info);

    map_switch_to_kernel(&mut allocate_pages, &mut page_map);

    // TODO: map the rest of available memory?
    info!("total memory mapped: {}B", page_map.size());

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

    let serial_controller_port = get_serial_controller(image_handle, system_table.boot_services());

    let (_system_table, _memory_map) =
        unsafe { system_table.exit_boot_services(MemoryType::LOADER_DATA) };

    unsafe { switch_to_kernel(page_map, serial_controller_port) }
}

unsafe fn switch_to_kernel(page_map: PageMap, serial_controller_port: u16) -> ! {
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
        "mov si, {2:x}",
        "jmp {0}",
        // See Note [The kernel's virtual address]
        in(reg) KERNEL_ENTRYPOINT,
        in(reg) PAGE_SIZE,
        in(reg) serial_controller_port
    );

    unreachable_unchecked()
}

struct KernelInfo {
    physical_address: u64,
    size: usize,
    allocated_pages: usize,
}

fn load_kernel(
    image_handle: Handle,
    system_table: &mut SystemTable<Boot>,
    kernel_file_name: &CStr16,
) -> Result<KernelInfo, uefi::Status> {
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

    Ok(KernelInfo {
        physical_address: kernel_addr,
        size: kernel_size,
        allocated_pages: kernel_pages,
    })
}

fn map_stack(allocate_pages: &mut dyn FnMut(usize) -> u64, page_map: &mut PageMap) {
    let stack_num_pages = (KERNEL_ENTRYPOINT as usize + PAGE_SIZE - 1) / PAGE_SIZE;

    // Assumes that the stack precedes the kernel in virtual address space.
    let stack_virtual_address = 0x0;
    let stack_physical_address = allocate_pages(stack_num_pages);

    let mut offset = 0;
    for _page in 0..stack_num_pages {
        page_map.set(
            allocate_pages,
            stack_virtual_address + offset,
            stack_physical_address + offset,
            PageMapFlags::W,
        );
        offset += PAGE_SIZE as u64;
    }
}

fn map_kernel_segment(
    allocate_pages: &mut dyn FnMut(usize) -> u64,
    page_map: &mut PageMap,
    segment_info: v0::SegmentInfo,
    segment_data: &[u8],
    flags: PageMapFlags,
) {
    let segment_pages = ((segment_info.size as usize) + PAGE_SIZE - 1) / PAGE_SIZE;

    let base_virtual_addr: u64 = segment_info.load_address;
    let base_physical_addr: u64 = allocate_pages(segment_pages);

    let mut offset: u64 = 0;
    for _page in 0..segment_pages {
        let page_virtual_addr = base_virtual_addr + offset;
        let page_physical_addr = base_physical_addr + offset;

        let page_buffer: &mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(page_physical_addr as *mut u8, PAGE_SIZE) };

        for i in 0..PAGE_SIZE {
            match segment_data.get(offset as usize + i).copied() {
                None => {
                    page_buffer[i] = 0;
                }
                Some(value) => {
                    page_buffer[i] = value;
                }
            }
        }

        page_map.set(allocate_pages, page_virtual_addr, page_physical_addr, flags);

        offset += PAGE_SIZE as u64;
    }
}

fn map_kernel(
    allocate_pages: &mut dyn FnMut(usize) -> u64,
    page_map: &mut PageMap,
    kernel_info: &KernelInfo,
) {
    let kernel_buffer: &[u8] = unsafe {
        core::slice::from_raw_parts(kernel_info.physical_address as *const u8, kernel_info.size)
    };

    let kernel_exe: v0::Exe =
        v0::Exe::parse(kernel_buffer).expect("kernel is not a v0 learn-os executable");

    map_kernel_segment(
        allocate_pages,
        page_map,
        kernel_exe.code_info(),
        kernel_exe.code(),
        PageMapFlags::X,
    );
    map_kernel_segment(
        allocate_pages,
        page_map,
        kernel_exe.rodata_info(),
        kernel_exe.rodata(),
        PageMapFlags::default(),
    );
    map_kernel_segment(
        allocate_pages,
        page_map,
        kernel_exe.rwdata_info(),
        kernel_exe.rwdata(),
        PageMapFlags::W,
    );

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
        PageMapFlags::X,
    );
    info!("finished setting up page map for context switch");
}

pub struct PciHeader {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u32,
    pub header_type: u8,
}

fn pci_header_read(
    pci_root_bridge: &PciRootBridgeIo,
    bus: u8,
    device: u8,
    function: u8,
) -> PciHeader {
    let vendor_and_device_ids = pci_root_bridge
        .pci_read_u32(PciConfigurationAddress {
            bus,
            device,
            function,
            register: 0x0,
        })
        .unwrap();

    let command = pci_root_bridge
        .pci_read_u32(PciConfigurationAddress {
            bus,
            device,
            function,
            register: 0x4,
        })
        .unwrap();

    let header_type = pci_root_bridge
        .pci_read_u8(PciConfigurationAddress {
            bus,
            device,
            function,
            register: 0xe,
        })
        .unwrap();

    PciHeader {
        vendor_id: (vendor_and_device_ids & 0xffff) as u16,
        device_id: (vendor_and_device_ids >> 16) as u16,
        command,
        header_type,
    }
}

/// Find the serial controller's I/O port via PCI.
fn get_serial_controller(image_handle: Handle, boot_services: &BootServices) -> u16 {
    let handle = boot_services
        .get_handle_for_protocol::<PciRootBridgeIo>()
        .unwrap();

    /* `open_protocol` is unsafe because it gives back a protocol interface that could be
    uninstalled by other code, invalidating the Rust reference. To reflect this, I'm using
    an `unsafe` block for the lifetime of `pci_root_bridge`.
    */
    unsafe {
        /* I originally tried to do this with `open_protocol_exclusive`, and the program hanged.

        Looking through the debugger I found that the call was returning `Err`, but the panic
        handler in the `unwrap` was looping instead of printing then crashing. I found that even
        `info!` logging broke after `open_protocol_exclusive` failed, even if I didn't `unwrap`.

        I had to go into the debugger to find the EFI status code. It was obscene decimal value
        but had a simple hex representation: `0x8000...000F`.
        The [EFI status codes reference](https://uefi.org/specs/UEFI/2.10/Apx_D_Status_Codes.html#status-codes)
        shows this is `EFI_ACCESS_DENIED`.

        [`OpenProtocol`](https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html?highlight=openprotocol#efi-boot-services-openprotocol)
        lists the status codes it returns and why. I realised that opening this protocol exclusively
        was the issue. There is probably a PCI Bus Driver running with driver-exclusive access to
        the protocol.
        */
        let pci_root_bridge = boot_services
            .open_protocol::<PciRootBridgeIo>(
                OpenProtocolParams {
                    handle,
                    agent: image_handle,
                    controller: None,
                },
                OpenProtocolAttributes::GetProtocol,
            )
            .unwrap();

        {
            let pci_header = pci_header_read(&pci_root_bridge, 0, 0, 0);

            // 0x8086 for Intel, woohoo!
            assert_eq!(pci_header.vendor_id, 0x8086);

            /* I was reading the 82371FB (PIIX) and 82371SB (PIIX3) datasheet because it
            was the first thing I saw on in the [440FX resources](https://web.archive.org/web/20041127232037/https://www.intel.com/design/archives/chipsets/440/index.htm),
            (QEMU's default chipset) with PCI in its name. So I was expecting to see 0x7000 because I
            thought I was on the PIIX3. I found 0x1237 instead.

            In the QEMU codebase, 0x1237 lead me to a constant named
            [`PCI_DEVICE_ID_INTEL_82441`](https://github.com/qemu/qemu/blob/dec9742cbc59415a8b83e382e7ae36395394e4bd/include/hw/pci/pci_ids.h#L241).
            I was looking at the wrong datasheet; the first listing in the 440FX resources
            is for the [82441FX PCI and Memory Controller](https://web.archive.org/web/20030706082243/http://intel.com/design/chipsets/datashts/29054901.pdf).
            Section 3.2.3 lists the device identification register (DID) with a default value of 0x1237.
            */
            assert_eq!(pci_header.device_id, 0x1237);
        }

        let serial_controller_pci_header = pci_header_read(&pci_root_bridge, 0, 3, 0);
        assert_eq!(serial_controller_pci_header.vendor_id, 0x1b36);
        assert_eq!(serial_controller_pci_header.device_id, 0x0002);

        let serial_controller_bar0_value = pci_root_bridge
            .pci_read_u32(PciConfigurationAddress {
                bus: 0,
                device: 3,
                function: 0,
                register: 0x10,
            })
            .unwrap();
        assert!(
            serial_controller_bar0_value <= u16::MAX as u32,
            "serial controller BAR0 is not a 16-bit value"
        );
        assert_eq!(
            serial_controller_bar0_value & 0x1,
            0x1,
            "serial controller BAR0 is not in I/O space"
        );

        let serial_controller_io_address: u16 = (serial_controller_bar0_value & 0xfffffff0) as u16;

        serial_controller_io_address
    }
}

fn pci_device_enumerate(pci_root_bridge: &PciRootBridgeIo) {
    /* I listed the available devices for QEMU's default machine, and got:

    * B0 D0 F0 - 8086:1237 (the 82441FX PMC)
    * B0 D1 F0 - 8086:7000 (the PIIX3)
    * B0 D2 F0 - 1234:1111 (QEMU standard VGA)
    * B0 D3 F0 - 8086:100e (82540EM ethernet controller - QEMU source code helps again)
    * B0 D4 F0 - 8086:24cd (82801DB ICH4)

    After twiddling the QEMU flags so that the network card is disabled and
    the USB keyboard is attached to the PIIX3, I just have the first 3.

    The PIIX3 is a multi-function device. The USB host controller is on
    bus 0, device 1, function 2, with `VID:DID = 8086:7020`.

    Weirdly, I also get a device with ID 7113. This is a power management
    controller from the PIIX4.
    */
    let print_pci_header = |bus, device, function, pci_header: &PciHeader| {
        info!("bus: {bus}, device: {device}, function: {function}");
        info!(
            "VID:DID {:x}:{:x}",
            pci_header.vendor_id, pci_header.device_id
        );
        info!("header type: {:#x}", pci_header.header_type);
    };

    for bus in 0..=255 {
        for device in 0..=31 {
            let function = 0;

            let pci_header = pci_header_read(pci_root_bridge, bus, device, function);
            if pci_header.vendor_id != 0xffff {
                print_pci_header(bus, device, function, &pci_header);

                if pci_header.header_type & 0x80 == 0x80 {
                    // multi-function device

                    for function in 1..=7 {
                        let pci_header = pci_header_read(pci_root_bridge, bus, device, function);
                        if pci_header.vendor_id != 0xffff {
                            print_pci_header(bus, device, function, &pci_header);
                        }
                    }
                }
            }
        }
    }
}

fn page_map_debug(page_map: &PageMap) {
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

fn alloc_growing_ref<T: ?Sized, E: core::fmt::Debug>(
    system_table: &mut SystemTable<Boot>,
    initial_size: usize,
    mut f: impl FnMut(&mut [u8]) -> uefi::Result<&mut T, E>,
) -> uefi::Result<PooledRef<T>, E> {
    let mut storage_size = initial_size;
    let mut storage_addr: NonNull<u8>;

    let mut buffer: &mut [u8];
    let buffer_typed: &mut T;

    let boot_services = system_table.boot_services();
    loop {
        storage_addr = boot_services
            .allocate_pool(MemoryType::BOOT_SERVICES_DATA, storage_size)
            .unwrap();
        buffer = unsafe { core::slice::from_raw_parts_mut(storage_addr.as_ptr(), storage_size) };

        info!("trying get_info with buffer size {}", storage_size);
        match f(buffer) {
            Ok(value) => {
                info!("alloc_growing_ref succeeded");
                buffer_typed = value;
                break;
            }
            Err(err) => {
                info!("alloc_growing_ref failed (buffer too small)");
                unsafe { boot_services.free_pool(storage_addr.as_ptr()).unwrap() };

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
        address: storage_addr.as_ptr(),
        typed_reference: buffer_typed,
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
            uefi::println!("  physical start: {:#x}", memory_map_entry.phys_start);
            uefi::println!("  virtual start: {:#x}", memory_map_entry.virt_start);
            uefi::println!("  page count: {:?}", memory_map_entry.page_count);
            uefi::println!("  attribute: {:?}", memory_map_entry.att);
        }
    })
}
