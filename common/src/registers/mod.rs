use core::arch::asm;

/** The CR0 register.

Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Section 2.5.
*/
#[derive(Clone, Copy)]
pub struct CR0(u64);

impl CR0 {
    /** Get the contents of the CR0 register.

    To set the contents of the CR0 register, use the [`CR0::write`] method.
    */
    pub fn read() -> Self {
        let value: u64;
        unsafe { asm!("mov {value}, cr0", value = out(reg) value) };
        Self(value)
    }

    /** Set the contents of the CR0 register.

    # Safety

    Refer to Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Section 2.5
    for the correct use of this register's fields.
    */
    pub unsafe fn write(&self) {
        asm!("mov cr0, {value}", value = in(reg) self.0)
    }

    /// Paging.
    pub fn pg(&self) -> bool {
        let mask = 1 << 31;
        self.0 & mask == mask
    }

    /// Protection enable.
    pub fn pe(&self) -> bool {
        let mask = 1;
        self.0 & mask == mask
    }
}

/** The contents of the CR3 register when used with 4-level paging and PCIDs disabled.

Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Table 4-12.
*/
#[derive(Clone, Copy)]
pub struct CR3(u64);

impl CR3 {
    /** Get the contents of the CR3 register.

    To set the contents of the CR3 register, use the [`CR3::write`] method.
    */
    pub fn read() -> Self {
        let mut value: u64;
        unsafe { asm!("mov {value}, cr3", value = out(reg) value) };
        Self(value)
    }

    /** Set the contents of the CR3 register.

    # Safety

    Extremely unsafe.

    The CR3 register controls address translation, which is used pervasively by the processor.
    Absolute addresses that are used after [`CR3::write`] must be valid with respect to
    the new virtual memory mapping.

    This includes:

    * The return address in [`CR3::write`]'s stack frame

      This function is marked `#[inline(always)]` because it's unlikely that the caller
      will want to preserve a return address.

    * The program counter

      Instruction fetches also uses address translation. After CR3 changes, the program
      counter still contains a virtual address that was valid according to the old memory
      map. If that address isn't mapped in the new memory map then instruction fetches
      will trigger page faults.

      Presumably you want the instructions that follow [`CR3::write`] to actually execute
      after CR3 is changed, which means the virtual addresses of these instructions need
      to be preserved by the new mapping, and they must map to the same physical memory
      as the old mapping. If the virtual addresses are preserved but they map different
      physical memory, then a new region of code will be executed after CR3 changes (kind
      of like a jump).

    * The frame pointer and stack pointer

      If the stack pointer is unmapped in the new virtual address space, then pushing to
      the stack will cause a page fault.

    * The IDT

      If the processor is responding to maskable interrupts then it will reload the IDT
      after changing the CR3 register. This triggers a page fault when the address in
      IDTR isn't preserved by the new memory map.
    */
    #[inline(always)]
    pub unsafe fn write(&self) {
        asm!("mov cr3, {value}", value = in(reg) self.0)
    }

    /// Page-level write-through.
    pub fn pwt(&self) -> bool {
        let mask = 1 << 3;
        (self.0 & mask) == mask
    }

    /// Set page-level write-through.
    pub fn set_pwt(&mut self, value: bool) {
        let mask = 1 << 3;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    /// Page-level cache disable.
    pub fn pcd(&self) -> bool {
        let mask = 1 << 4;
        (self.0 & mask) == mask
    }

    /// Set page-level cache disable.
    pub fn set_pcd(&mut self, value: bool) {
        let mask = 1 << 4;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    /// Physical address of the 4KiB aligned PML4 table.
    pub fn address(&self) -> u64 {
        self.0 & !0xfff
    }

    /// Set PML4 table address.
    pub fn set_address(&mut self, value: u64) {
        assert_eq!(
            value & !0xfff,
            value,
            "address {:#x} isn't 4KiB aligned",
            value
        );
        self.0 &= 0xfff;
        self.0 |= value;
    }
}

/** The CR4 register.

Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Section 2.5.
*/
#[derive(Clone, Copy)]
pub struct CR4(u64);

impl CR4 {
    /** Get the contents of the CR4 register.

    To set the contents of the CR4 register, use the [`CR4::write`] method.
    */
    pub fn read() -> Self {
        let value: u64;
        unsafe { asm!("mov {value}, cr4", value = out(reg) value) };
        Self(value)
    }

    /** Set the contents of the CR4 register.

    # Safety

    Refer to Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Section 2.5
    for the correct use of this register's fields.
    */
    pub unsafe fn write(&self) {
        asm!("mov cr4, {value}", value = in(reg) self.0)
    }

    /// Physical address extension.
    pub fn pae(&self) -> bool {
        let mask = 1 << 5;
        self.0 & mask == mask
    }

    /// 57-bit linear addresses.
    pub fn la57(&self) -> bool {
        let mask = 1 << 12;
        self.0 & mask == mask
    }
}

/** The IA32_EFER MSR.

References:

* Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Section 2.8.7. (working with MSRs)
* Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 4, Table 2-2. (MSR details)
*/
#[allow(non_camel_case_types)]
pub struct IA32_EFER(u64);

impl IA32_EFER {
    const REGISTER_ADDRESS: u32 = 0xc000_0080;

    pub fn read() -> Self {
        let value_low: u32;
        let value_high: u32;

        unsafe {
            asm!(
                "rdmsr",
                in("ecx") Self::REGISTER_ADDRESS,
                out("edx") value_high,
                out("eax") value_low,
            );
        }

        let mut value = value_high as u64;
        value <<= 32;
        value |= value_low as u64;

        Self(value)
    }

    /// IA-32e Mode Enable.
    pub fn lme(&self) -> bool {
        let mask = 1 << 8;
        self.0 & mask == mask
    }
}
