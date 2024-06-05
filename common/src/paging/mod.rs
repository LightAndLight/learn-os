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

#[repr(C)]
pub struct PageMap {
    /// The page map's physical address.
    address: u64,

    /// The page size used in the map.
    page_size: usize,
}

impl PageMap {
    pub fn new(allocate_pages: &mut dyn FnMut(usize) -> u64, page_size: usize) -> Self {
        let pml4_address: u64 = allocate_pages(1);
        unsafe {
            init_memory(pml4_address as *mut u64, 512, 0);
        }

        PageMap {
            address: pml4_address,
            page_size,
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
                        total += self.page_size;
                    }
                }
            }
        }

        total
    }

    pub fn pml4_mut(&mut self) -> &mut [PML4E; 512] {
        unsafe { core::mem::transmute(self.address) }
    }

    pub fn pml4(&self) -> &[PML4E; 512] {
        unsafe { core::mem::transmute(self.address) }
    }

    /** Map a virtual page address to a physical page address.

    The mapped page is read-only.
    */
    pub fn set(
        &mut self,
        allocate_pages: &mut dyn FnMut(usize) -> u64,
        virtual_page_address: u64,
        physical_page_address: u64,
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

        let page_map_indices = address_to_page_map_indices(virtual_page_address);

        // This could be better. `present()` followed by `unwrap()` looks like an antipattern.
        let pml4 = self.pml4_mut();
        let pml4e: &mut PML4E = &mut pml4[page_map_indices.pml4];
        if !pml4e.present() {
            let pdpt_address = allocate_pages(1);
            unsafe {
                init_memory(pdpt_address as *mut u64, 512, 0);
            }
            *pml4e = PML4E::new(false, pdpt_address, false, false, false, false);
        }

        let pdpt = pml4e.pdpt_mut().unwrap();
        let pdpte = &mut pdpt[page_map_indices.pdpt];
        if !pdpte.present() {
            let pd_address = allocate_pages(1);
            unsafe {
                init_memory(pd_address as *mut u64, 512, 0);
            }
            *pdpte = PDPTE::new(false, pd_address, false, false, false, false);
        }

        let pd = pdpte.pd_mut().unwrap();
        let pde = &mut pd[page_map_indices.pd];
        if !pde.present() {
            let pt_address = allocate_pages(1);
            unsafe {
                init_memory(pt_address as *mut u64, 512, 0);
            }
            *pde = PDE::new(false, pt_address, false, false, false, false);
        }

        let pt = pde.pt_mut().unwrap();
        pt[page_map_indices.pt] =
            PTE::new(false, physical_page_address, false, false, false, false);
    }

    /** Map a virtual page address to a physical page address.

    The mapped page is writable.
    */
    pub fn set_writable(
        &mut self,
        allocate_pages: &mut dyn FnMut(usize) -> u64,
        virtual_page_address: u64,
        physical_page_address: u64,
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

        /* This is the same code as `PageMap::set`, except the all page table levels have
        to be marked as writable for the virtual page to be writable.
        */

        let page_map_indices = address_to_page_map_indices(virtual_page_address);

        // This could be better. `present()` followed by `unwrap()` looks like an antipattern.
        let pml4 = self.pml4_mut();
        let pml4e: &mut PML4E = &mut pml4[page_map_indices.pml4];
        if !pml4e.present() {
            let pdpt_address = allocate_pages(1);
            unsafe {
                init_memory(pdpt_address as *mut u64, 512, 0);
            }
            *pml4e = PML4E::new(false, pdpt_address, false, false, false, false);
        }
        pml4e.set_writable(true);

        let pdpt = pml4e.pdpt_mut().unwrap();
        let pdpte = &mut pdpt[page_map_indices.pdpt];
        if !pdpte.present() {
            let pd_address = allocate_pages(1);
            unsafe {
                init_memory(pd_address as *mut u64, 512, 0);
            }
            *pdpte = PDPTE::new(false, pd_address, false, false, false, false);
        }
        pdpte.set_writable(true);

        let pd = pdpte.pd_mut().unwrap();
        let pde = &mut pd[page_map_indices.pd];
        if !pde.present() {
            let pt_address = allocate_pages(1);
            unsafe {
                init_memory(pt_address as *mut u64, 512, 0);
            }
            *pde = PDE::new(false, pt_address, false, false, false, false);
        }
        pde.set_writable(true);

        let pt = pde.pt_mut().unwrap();
        pt[page_map_indices.pt] = PTE::new(false, physical_page_address, false, false, false, true);
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
#[derive(Clone, Copy)]
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

    /// Get an exclusive reference to the PDPT pointed to by this entry.
    pub fn pdpt_mut(&mut self) -> Option<&mut [PDPTE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    (self.0 & !0xfff) as *mut PDPTE,
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
                    (self.0 & !0xfff) as *mut PDPTE,
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
#[derive(Clone, Copy)]
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

    /// Get an exclusive reference to the PD pointed to by this entry.
    pub fn pd_mut(&mut self) -> Option<&mut [PDE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    (self.0 & !0xfff) as *mut PDE,
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
                    (self.0 & !0xfff) as *const PDE,
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
#[derive(Clone, Copy)]
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

    /// Get an exclusive reference to the PT pointed to by this entry.
    pub fn pt_mut(&mut self) -> Option<&mut [PTE]> {
        if self.present() {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    (self.0 & !0xfff) as *mut PTE,
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
                    (self.0 & !0xfff) as *const PTE,
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
#[derive(Clone, Copy)]
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

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn present(&self) -> bool {
        self.0 & 1 == 1
    }
}
