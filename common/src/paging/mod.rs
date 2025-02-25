use core::ops::BitOr;

use crate::registers::CR3;

#[derive(Debug)]
struct PageMapIndices {
    pml4: usize,
    pdpt: usize,
    pd: usize,
    pt: usize,
}

fn address_to_page_map_indices(virtual_address: u64) -> PageMapIndices {
    let mut offset = 12 + 3 * 9;
    let mut mask = 0b111111111 << offset;
    let pml4_index: u64 = (virtual_address & mask) >> offset;

    offset -= 9;
    mask >>= 9;
    let pdpt_index: u64 = (virtual_address & mask) >> offset;

    offset -= 9;
    mask >>= 9;
    let pd_index: u64 = (virtual_address & mask) >> offset;

    offset -= 9;
    mask >>= 9;
    let pt_index: u64 = (virtual_address & mask) >> offset;

    PageMapIndices {
        pml4: pml4_index as usize,
        pdpt: pdpt_index as usize,
        pd: pd_index as usize,
        pt: pt_index as usize,
    }
}

fn page_map_indices_to_address(indices: PageMapIndices) -> u64 {
    let mut value: u64 = 0;
    value |= (indices.pml4 as u64) << (12 + 3 * 9);
    value |= (indices.pdpt as u64) << (12 + 2 * 9);
    value |= (indices.pd as u64) << (12 + 9);
    value |= (indices.pt as u64) << 12;
    value
}

unsafe fn init_memory<T: Copy>(data: *mut T, len: usize, value: T) {
    let entries = core::slice::from_raw_parts_mut(data, len).iter_mut();
    for entry in entries {
        *entry = value;
    }
}

/** Memory mapping permissions.

The default (`PageMapFlags::default()`) is read-only. Use the associated constants
with bitwise OR to add more permissions.

## Example

```rust
let rwx = PageMapFlags::W | PageMapFlags::X;
```
*/
#[derive(Default, Clone, Copy)]
pub struct PageMapFlags {
    writeable: bool,
    executable: bool,
}

impl PageMapFlags {
    pub const W: PageMapFlags = PageMapFlags {
        writeable: true,
        executable: false,
    };

    pub const X: PageMapFlags = PageMapFlags {
        writeable: false,
        executable: true,
    };
}

impl BitOr for PageMapFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            writeable: self.writeable || rhs.writeable,
            executable: self.executable || rhs.executable,
        }
    }
}

pub enum PageMapMode {
    /// The page map is currently being used for address translation.
    Active,

    /// The page map is not being used for address translation.
    Inactive,
}

/// A 4-level, recursively-mapped page table structure for x86-64.
#[repr(C)]
pub struct PageMap {
    /// The page map's memory address.
    address: u64,
}

impl PageMap {
    pub const PAGE_SIZE: usize = 4096;

    pub fn new(allocate_pages: &mut dyn FnMut(usize) -> u64) -> Self {
        let pml4_address: u64 = allocate_pages(1);
        unsafe {
            init_memory(pml4_address as *mut u64, 512, 0);
        }

        PageMap {
            address: pml4_address,
        }
    }

    /// Read the page table assigned to the [`CR3`] register.
    pub fn from_cr3() -> Self {
        let cr3 = CR3::read();
        Self {
            address: cr3.address(),
        }
    }

    pub fn address(&self) -> u64 {
        self.address
    }

    /// The total amount of mapped memory, in bytes.
    pub fn size(&self) -> usize {
        let mut total = 0;

        for pdpt in self.pml4().iter().filter_map(|pml4e| pml4e.pdpt()) {
            for pd in pdpt.iter().filter_map(|pdpte| pdpte.pd()) {
                for pt in pd.iter().filter_map(|pde| pde.pt()) {
                    for _pte in pt.iter().filter(|pte| pte.present()) {
                        total += Self::PAGE_SIZE;
                    }
                }
            }
        }

        total
    }

    /** Get a mutable pointer to the PML4.

    * [`PageMapMode::Active`] - access the PML4 via recursive mapping
    * [`PageMapMode::Inactive`] - access the PML4 using the page table's underlying address

    # Safety

    * [`PageMapMode::Active`] - the page table must be in use for address translation.
    * [`PageMapMode::Inactive`] - the page table's underlying address must be mapped.

    When either of these conditions aren't met, the pointer will be invalid and dereferencing
    it trigger a page fault.
    */
    pub fn pml4_mut(&mut self, mode: PageMapMode) -> *mut [PML4E; 512] {
        match mode {
            PageMapMode::Active => {
                /* Note [Recursive mapping]

                Index 511 (the 512th element) of the PML4 is mapped to itself instead of
                a PML4 entry.

                During address translation, the 9 most significant bits are used as the index
                into the PML4, and the 9 bits after that are normally an index into the PDPT.
                The recursive mapping means that starting an address with `0b111_111_111`
                causes the next 9 bits to index into the PML4 instead of a PDPT. Another
                `0b111_111_111` brings us back to the PML4 again, when we'd normally have a
                PD. And one more `0b111_111_111` to get back to the PML4 when we'd normally
                have a PT.

                At this point there are 12 bits of the address remaining, which are normally
                used as the offset into the 4KiB page that has been retrieved by delving into
                the page map. The recursive mapping process has turned up the address of the
                PML4 instead. It is still a 4KiB page, so the remaining 12 bits are used
                to index into the PML4, retrieving a PML4 entry.

                Putting this all together, when a PML4 is recursively mapped at index 511
                and is being used for address translation, its virtual memory address is
                9 ones, then 9 ones, then 9 ones, then 12 zeroes:

                ```
                0b111_111_111___111_111_111___111_111_111___000_000_000_000

                = 0b0111_1111_1111_1111_1111_1111_1111_0000_0000_0000

                = 0x7_f_f_f_f_f_f_0_0_0

                = 0x007f_ffff_f000
                ```
                */

                0x0007_ffff_f000 as *mut [PML4E; 512]
            }
            PageMapMode::Inactive => self.address as *mut [PML4E; 512],
        }
    }

    /** Get an immutable pointer to the PML4.

    * [`PageMapMode::Active`] - access the PML4 via recursive mapping
    * [`PageMapMode::Inactive`] - access the PML4 using the page table's underlying address

    # Safety

    * [`PageMapMode::Active`] - the page table must be in use for address translation.
    * [`PageMapMode::Inactive`] - the page table's underlying address must be mapped.

    When either of these conditions aren't met, the pointer will be invalid and dereferencing
    it trigger a page fault.
    */
    pub fn pml4(&self, mode: PageMapMode) -> *const [PML4E; 512] {
        match mode {
            PageMapMode::Active => {
                // See Note [Recursive mapping]
                0x0007_ffff_f000 as *const [PML4E; 512]
            }
            PageMapMode::Inactive => self.address as *mut [PML4E; 512],
        }
    }

    /** Get a mutable pointer to a PDP table.

    * [`PageMapMode::Active`] - access the PDP via recursive mapping
    * [`PageMapMode::Inactive`] - access the PDP using the page table's underlying address

    [`PageMap::pml4_mut`] returns PML4 entries, which contain physical addresses of PDP tables.
    Those physical addresses are not likely to be mapped in an arbitrary page table configuration.
    When that's the case, PDP tables can still be accessed via recursive mapping.

    # Safety

    * [`PageMapMode::Active`] - the page table must be in use for address translation.
    * [`PageMapMode::Inactive`] - the physical addresses in the page table must be mapped.

    When either of these conditions aren't met, the pointer will be invalid and dereferencing
    it trigger a page fault, or the function itself will cause a page fault.
    */
    pub unsafe fn pdpt_mut(&mut self, mode: PageMapMode, pml4_index: u16) -> *mut [PDPTE; 512] {
        // TODO: does this fail on `no_std` (formatting the number)?
        assert!(pml4_index < (1 << 9), "PML4 index {} > 511", pml4_index);

        match mode {
            PageMapMode::Active => {
                /* Note [PDP tables via recursive mapping]

                ```
                  Get the PML4
                  |             Get the PML4
                  |             |             Index into PML4
                  |             |             |             Offset into PDPT
                  |             |             |             |
                  v~~~~~~~~~~   v~~~~~~~~~~   v~~~~~~~~~~   v~~~~~~~~~~~~~~
                0b111_111_111___111_111_111___xxx_xxx_xxx___000_000_000_000
                ```

                ```
                0b111_111_111___111_111_111___xxx_xxx_xxx___000_000_000_000

                = 0b0111_1111_1111_1111_111x_xxxx_xxxx_0000_0000_0000

                = 0x0111_1111_1111_1111_1110_0000_0000_0000_0000_0000 | (0b000x_xxxx_xxxx << 12)

                = 0x007f_ffe0_0000 | (0b000x_xxxx_xxxx << 12)
                ```

                See also: Note [Recursive mapping]
                */
                (0x007f_ffe0_0000_u64 | (pml4_index as u64)) as *mut [PDPTE; 512]
            }
            PageMapMode::Inactive => {
                let pml4e = &mut (*self.pml4_mut(PageMapMode::Inactive))[pml4_index as usize];
                pml4e.pdpt_address() as *mut [PDPTE; 512]
            }
        }
    }

    /** Get an immutable mutable pointer to a PDP table.

    * [`PageMapMode::Active`] - access the PDP via recursive mapping
    * [`PageMapMode::Inactive`] - access the PDP using the page table's underlying address

    [`PageMap::pml4_mut`] returns PML4 entries, which contain physical addresses of PDP tables.
    Those physical addresses are not likely to be mapped in an arbitrary page table configuration.
    When that's the case, PDP tables can still be accessed via recursive mapping.

    # Safety

    * [`PageMapMode::Active`] - the page table must be in use for address translation.
    * [`PageMapMode::Inactive`] - the physical addresses in the page table must be mapped.

    When either of these conditions aren't met, the pointer will be invalid and dereferencing
    it trigger a page fault, or the function itself will cause a page fault.
    */
    pub unsafe fn pdpt(&self, mode: PageMapMode, pml4_index: u16) -> *const [PDPTE; 512] {
        assert!(pml4_index < (1 << 9), "PML4 index {} > 511", pml4_index);

        match mode {
            PageMapMode::Active => {
                // See Note [PDP tables via recursive mapping]
                (0x0007_ffe0_0000_u64 | (pml4_index as u64)) as *mut [PDPTE; 512]
            }
            PageMapMode::Inactive => {
                let pml4e = &(*self.pml4(PageMapMode::Inactive))[pml4_index as usize];
                pml4e.pdpt_address() as *mut [PDPTE; 512]
            }
        }
    }

    /** Get a mutable pointer to a PD table.

    * [`PageMapMode::Active`] - access the PD via recursive mapping
    * [`PageMapMode::Inactive`] - access the PD using the page table's underlying address

    [`PageMap::pdp_mut`] returns PDP entries, which contain physical addresses of PD tables.
    Those physical addresses are not likely to be mapped in an arbitrary page table configuration.
    When that's the case, PD tables can still be accessed via recursive mapping.

    # Safety

    * [`PageMapMode::Active`] - the page table must be in use for address translation.
    * [`PageMapMode::Inactive`] - the physical addresses in the page table must be mapped.

    When either of these conditions aren't met, the pointer will be invalid and dereferencing
    it trigger a page fault, or the function itself will cause a page fault.
    */
    pub unsafe fn pd_mut(
        &mut self,
        mode: PageMapMode,
        pml4_index: u16,
        pdpt_index: u16,
    ) -> *mut [PDE; 512] {
        // TODO: does this fail on `no_std` (formatting the number)?
        assert!(pml4_index < (1 << 9), "PML4 index {} > 511", pml4_index);
        assert!(pdpt_index < (1 << 9), "PDPT index {} > 511", pdpt_index);

        match mode {
            PageMapMode::Active => {
                /* See Note [Recursive mapping] and Note [PDP tables via recursive mapping] for an
                explanation of how this works.

                ```
                  Get the PML4
                  |             Index into PML4
                  |             |             Index into PDPT
                  |             |             |             Offset into PD
                  |             |             |             |
                  v~~~~~~~~~~   v~~~~~~~~~~   v~~~~~~~~~~   v~~~~~~~~~~~~~~
                0b111_111_111___xxx_xxx_xxx___yyy_yyy_yyy___000_000_000_000
                ```

                ```
                0b111_111_111___xxx_xxx_xxx___yyy_yyy_yyy___000_000_000_000

                = 0b0111_1111_11xx_xxxx_xxxy_yyyy_yyyy_0000_0000_0000

                = 0b0111_1111_1100_0000_0000_0000_0000_0000_0000_0000 | (0b00xx_xxxx_xxx0 << 12 + 9) | (0b000y_yyyy_yyyy << 12)

                = 0x007f_c000_0000 | (0b00xx_xxxx_xxx0 << 12 + 9) | (0b000y_yyyy_yyyy << 12)
                ```

                See also: Note [Recursive mapping]
                */
                let mut mask = pml4_index as u64;
                mask << 9;
                mask |= pdpt_index as u64;
                mask << 12;

                (0x007f_c000_0000_u64 | mask) as *mut [PDE; 512]
            }
            PageMapMode::Inactive => {
                let pdpte =
                    &mut (*self.pdpt_mut(PageMapMode::Inactive, pml4_index))[pdpt_index as usize];
                pdpte.pd_address() as *mut [PDE; 512]
            }
        }
    }

    /// Map a virtual page address to a physical page address.
    pub fn set(
        &mut self,
        allocate_pages: &mut dyn FnMut(usize) -> u64,
        virtual_page_address: u64,
        physical_page_address: u64,
        flags: PageMapFlags,
    ) {
        assert_eq!(
            virtual_page_address & !0xfff,
            virtual_page_address,
            "virtual address {:#x} isn't 4KiB aligned",
            virtual_page_address
        );

        assert_eq!(
            physical_page_address & !0xfff,
            physical_page_address,
            "physical address {:#x} isn't 4KiB aligned",
            physical_page_address
        );

        // All levels of the page table are created in read-only mode.
        let default_execute_disable = true;
        let default_writeable = false;

        // The requested permissions for this page
        let writeable = flags.writeable;
        let executable = flags.executable;

        let page_map_indices = address_to_page_map_indices(virtual_page_address);

        // This could be better. `present()` followed by `unwrap()` looks like an antipattern.
        let pml4 = self.pml4_mut();
        let pml4e: &mut PML4E = &mut pml4[page_map_indices.pml4];
        if !pml4e.present() {
            let pdpt_address = allocate_pages(1);
            unsafe {
                init_memory(pdpt_address as *mut u64, 512, 0);
            }

            *pml4e = PML4E::new(
                default_execute_disable,
                pdpt_address,
                false,
                false,
                false,
                default_writeable,
            );
        }
        if writeable {
            pml4e.set_writable(true);
        }
        if executable {
            pml4e.set_execute_disable(false);
        }

        let pdpt = pml4e.pdpt_mut().unwrap();
        let pdpte = &mut pdpt[page_map_indices.pdpt];
        if !pdpte.present() {
            let pd_address = allocate_pages(1);
            unsafe {
                init_memory(pd_address as *mut u64, 512, 0);
            }
            *pdpte = PDPTE::new(
                default_execute_disable,
                pd_address,
                false,
                false,
                false,
                default_writeable,
            );
        }
        if writeable {
            pdpte.set_writable(true);
        }
        if executable {
            pdpte.set_execute_disable(false);
        }

        let pd = pdpte.pd_mut().unwrap();
        let pde = &mut pd[page_map_indices.pd];
        if !pde.present() {
            let pt_address = allocate_pages(1);
            unsafe {
                init_memory(pt_address as *mut u64, 512, 0);
            }
            *pde = PDE::new(
                default_execute_disable,
                pt_address,
                false,
                false,
                false,
                default_writeable,
            );
        }
        if writeable {
            pde.set_writable(true);
        }
        if executable {
            pde.set_execute_disable(false);
        }

        let pt = pde.pt_mut().unwrap();
        pt[page_map_indices.pt] = PTE::new(
            !executable,
            physical_page_address,
            false,
            false,
            false,
            writeable,
        );
    }

    /// Unmap a virtual page address.
    pub fn unset(&mut self, virtual_page_address: u64) {
        assert_eq!(
            virtual_page_address & !0xfff,
            virtual_page_address,
            "virtual address {:#x} isn't 4KiB aligned",
            virtual_page_address
        );

        let indices = address_to_page_map_indices(virtual_page_address);

        let pml4 = self.pml4_mut();
        let pml4e = &mut pml4[indices.pml4];

        let pdpt = match pml4e.pdpt_mut() {
            None => {
                return;
            }
            Some(pdpt) => pdpt,
        };
        let pdpte = &mut pdpt[indices.pdpt];

        let pd = match pdpte.pd_mut() {
            None => {
                return;
            }
            Some(pd) => pd,
        };
        let pde = &mut pd[indices.pd];

        let pt = match pde.pt_mut() {
            None => {
                return;
            }
            Some(pt) => pt,
        };

        pt[indices.pt] = PTE::unset();
    }

    pub fn debug(
        &self,
        debug_pml4e: &mut dyn FnMut(usize, &PML4E),
        debug_pdpte: &mut dyn FnMut(usize, &PDPTE),
        debug_pde: &mut dyn FnMut(usize, &PDE),
        debug_pte: &mut dyn FnMut(usize, u64, &PTE),
    ) {
        for (pml4_index, pml4e) in self.pml4().iter().enumerate() {
            if let Some(pdpt) = pml4e.pdpt() {
                debug_pml4e(pml4_index, pml4e);

                for (pdpt_index, pdpte) in pdpt.iter().enumerate() {
                    if let Some(pd) = pdpte.pd() {
                        debug_pdpte(pdpt_index, pdpte);

                        for (pd_index, pde) in pd.iter().enumerate() {
                            if let Some(pt) = pde.pt() {
                                debug_pde(pd_index, pde);

                                for (pt_index, pte) in pt.iter().enumerate() {
                                    if pte.present() {
                                        debug_pte(
                                            pt_index,
                                            page_map_indices_to_address(PageMapIndices {
                                                pml4: pml4_index,
                                                pdpt: pdpt_index,
                                                pd: pd_index,
                                                pt: pt_index,
                                            }),
                                            pte,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** A PML4 entry.

Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Table 4-15 and Figure 4-11.
*/
pub struct PML4E(u64);

impl PML4E {
    pub fn new(
        execute_disable: bool,
        pdpt_address: u64,
        pcd: bool,
        pwt: bool,
        user: bool,
        writable: bool,
    ) -> Self {
        assert!(
            // `0xfff` is `0b111111111`.
            pdpt_address & 0xfff == 0,
            "address {:#x} is not 4KiB aligned",
            pdpt_address
        );

        assert!(
            pdpt_address & 0xfff << 51 == 0,
            "address {:#x} uses more than 52 bits",
            pdpt_address
        );

        let mut value = pdpt_address;

        if execute_disable {
            value |= 1 << 63;
        }

        if pcd {
            value |= 1 << 4;
        }

        if pwt {
            value |= 1 << 3;
        }

        if user {
            value |= 1 << 2;
        }

        if writable {
            value |= 1 << 1;
        }

        // The present bit.
        value |= 1;

        Self(value)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn set_execute_disable(&mut self, value: bool) {
        let mask = 1 << 63;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    pub fn present(&self) -> bool {
        self.0 & 1 == 1
    }

    pub fn writable(&self) -> bool {
        let mask = 0b10;
        self.0 & mask == mask
    }

    pub fn set_writable(&mut self, value: bool) {
        let mask = 0b10;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    fn pdpt_address(&self) -> u64 {
        let mask = (1 << 63) | 0xfff;
        self.0 & !mask
    }

    /// Get an exclusive reference to the PDPT pointed to by this entry.
    pub fn pdpt_mut(&mut self) -> Option<&mut [PDPTE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    self.pdpt_address() as *mut PDPTE,
                    512,
                ))
            }
        } else {
            None
        }
    }

    /// Get a shared reference to the PDPT pointed to by this entry.
    pub fn pdpt(&self) -> Option<&[PDPTE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts(
                    self.pdpt_address() as *const PDPTE,
                    512,
                ))
            }
        } else {
            None
        }
    }
}

/** A Page Directory Pointer Table entry.

Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Table 4-17 and Figure 4-11.
*/
pub struct PDPTE(u64);

impl PDPTE {
    pub fn new(
        execute_disable: bool,
        pd_address: u64,
        pcd: bool,
        pwt: bool,
        user: bool,
        writable: bool,
    ) -> Self {
        assert!(
            // `0xfff` is `0b111111111`.
            pd_address & 0xfff == 0,
            "address {:#x} is not 4KiB aligned",
            pd_address
        );

        assert!(
            pd_address & 0xfff << 51 == 0,
            "address {:#x} uses more than 52 bits",
            pd_address
        );

        let mut value = pd_address;

        if execute_disable {
            value |= 1 << 63;
        }

        if pcd {
            value |= 1 << 4;
        }

        if pwt {
            value |= 1 << 3;
        }

        if user {
            value |= 1 << 2;
        }

        if writable {
            value |= 1 << 1;
        }

        // The present bit.
        value |= 1;

        Self(value)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn set_execute_disable(&mut self, value: bool) {
        let mask = 1 << 63;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    pub fn present(&self) -> bool {
        self.0 & 1 == 1
    }

    pub fn writable(&self) -> bool {
        let mask = 0b10;
        self.0 & mask == mask
    }

    pub fn set_writable(&mut self, value: bool) {
        let mask = 0b10;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    fn pd_address(&self) -> u64 {
        let mask = (1 << 63) | 0xfff;
        self.0 & !mask
    }

    /// Get an exclusive reference to the PD pointed to by this entry.
    pub fn pd_mut(&mut self) -> Option<&mut [PDE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    self.pd_address() as *mut PDE,
                    512,
                ))
            }
        } else {
            None
        }
    }

    /// Get a shared reference to the PD pointed to by this entry.
    pub fn pd(&self) -> Option<&[PDE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts(
                    self.pd_address() as *const PDE,
                    512,
                ))
            }
        } else {
            None
        }
    }
}

/** A Page Directory entry.

Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Table 4-19 and Figure 4-11.
*/
pub struct PDE(u64);

impl PDE {
    pub fn new(
        execute_disable: bool,
        pdp_table_address: u64,
        pcd: bool,
        pwt: bool,
        user: bool,
        writable: bool,
    ) -> Self {
        assert!(
            // `0xfff` is `0b111111111`.
            pdp_table_address & 0xfff == 0,
            "address {:#x} is not 4KiB aligned",
            pdp_table_address
        );

        assert!(
            pdp_table_address & 0xfff << 51 == 0,
            "address {:#x} uses more than 52 bits",
            pdp_table_address
        );

        let mut value = pdp_table_address;

        if execute_disable {
            value |= 1 << 63;
        }

        if pcd {
            value |= 1 << 4;
        }

        if pwt {
            value |= 1 << 3;
        }

        if user {
            value |= 1 << 2;
        }

        if writable {
            value |= 1 << 1;
        }

        // The present bit.
        value |= 1;

        Self(value)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn set_execute_disable(&mut self, value: bool) {
        let mask = 1 << 63;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    pub fn present(&self) -> bool {
        self.0 & 1 == 1
    }

    pub fn writable(&self) -> bool {
        let mask = 0b10;
        self.0 & mask == mask
    }

    pub fn set_writable(&mut self, value: bool) {
        let mask = 0b10;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    fn pt_address(&self) -> u64 {
        let mask = (1 << 63) | 0xfff;
        self.0 & !mask
    }

    /// Get an exclusive reference to the PT pointed to by this entry.
    pub fn pt_mut(&mut self) -> Option<&mut [PTE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    self.pt_address() as *mut PTE,
                    512,
                ))
            }
        } else {
            None
        }
    }

    /// Get a shared reference to the PT pointed to by this entry.
    pub fn pt(&self) -> Option<&[PTE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts(
                    self.pt_address() as *const PTE,
                    512,
                ))
            }
        } else {
            None
        }
    }
}

/** A Page Table entry.

Reference: Intel® 64 and IA-32 Architectures Software Developer’s Manual, Vol 3A, Table 4-20 and Figure 4-11.
*/
pub struct PTE(u64);

impl PTE {
    pub fn new(
        execute_disable: bool,
        page_address: u64,
        pcd: bool,
        pwt: bool,
        user: bool,
        writable: bool,
    ) -> Self {
        assert!(
            // `0xfff` is `0b111111111`.
            page_address & 0xfff == 0,
            "address {:#x} is not 4KiB aligned",
            page_address
        );

        assert!(
            page_address & 0xfff << 51 == 0,
            "address {:#x} uses more than 52 bits",
            page_address
        );

        let mut value = page_address;

        if execute_disable {
            value |= 1 << 63;
        }

        if pcd {
            value |= 1 << 4;
        }

        if pwt {
            value |= 1 << 3;
        }

        if user {
            value |= 1 << 2;
        }

        if writable {
            value |= 1 << 1;
        }

        // The present bit.
        value |= 1;

        Self(value)
    }

    pub fn unset() -> Self {
        Self(0)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn present(&self) -> bool {
        self.0 & 1 == 1
    }
}
