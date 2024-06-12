#![no_std]

pub mod raw;

pub use raw::{PciIoProtocolWidth, PciRootBridgeIoProtocolWidth};

use raw::{PciIoProtocol, PciRootBridgeIoProtocol};
use uefi::{proto::unsafe_protocol, StatusExt};

#[derive(Debug)]
#[repr(transparent)]
#[unsafe_protocol(PciRootBridgeIoProtocol::GUID)]
pub struct PciRootBridgeIo(PciRootBridgeIoProtocol);

impl PciRootBridgeIo {
    pub unsafe fn pci_read(
        &self,
        width: PciRootBridgeIoProtocolWidth,
        address: PciConfigurationAddress,
        count: usize,
        buffer: *mut u8,
    ) -> uefi::Result {
        (self.0.pci.read)(&self.0, width, address.to_u64(), count, buffer).to_result()
    }

    pub unsafe fn pci_write(
        &self,
        width: PciRootBridgeIoProtocolWidth,
        address: PciConfigurationAddress,
        count: usize,
        buffer: *mut u8,
    ) -> uefi::Result {
        (self.0.pci.write)(&self.0, width, address.to_u64(), count, buffer).to_result()
    }

    pub fn pci_read_u8(&self, address: PciConfigurationAddress) -> uefi::Result<u8> {
        let mut value: u8 = 0;
        unsafe {
            self.pci_read(
                PciRootBridgeIoProtocolWidth::Uint8,
                address,
                1,
                &mut value as *mut u8,
            )
        }?;
        Ok(value)
    }

    pub fn pci_read_u16(&self, address: PciConfigurationAddress) -> uefi::Result<u16> {
        let mut value: u16 = 0;
        unsafe {
            self.pci_read(
                PciRootBridgeIoProtocolWidth::Uint16,
                address,
                1,
                &mut value as *mut u16 as *mut u8,
            )
        }?;
        Ok(value)
    }

    pub fn pci_read_u32(&self, address: PciConfigurationAddress) -> uefi::Result<u32> {
        let mut value: u32 = 0;
        unsafe {
            self.pci_read(
                PciRootBridgeIoProtocolWidth::Uint32,
                address,
                1,
                &mut value as *mut u32 as *mut u8,
            )
        }?;
        Ok(value)
    }

    pub fn pci_write_u32(&self, address: PciConfigurationAddress, mut value: u32) -> uefi::Result {
        unsafe {
            self.pci_write(
                PciRootBridgeIoProtocolWidth::Uint32,
                address,
                1,
                &mut value as *mut u32 as *mut u8,
            )
        }
    }

    pub fn configuration(&self) -> uefi::Result<Descriptors> {
        let mut value: *const u8 = core::ptr::null();
        unsafe { (self.0.configuration)(&self.0, &mut value) }.to_result()?;
        Ok(Descriptors {
            _owner: self,
            data: value,
        })
    }
}

pub struct PciConfigurationAddress {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub register: u8,
}

impl PciConfigurationAddress {
    pub fn to_u64(&self) -> u64 {
        let mut value: u64 = self.register as u64;
        value |= (self.function as u64) << 8;
        value |= (self.device as u64) << 16;
        value |= (self.bus as u64) << 24;
        value
    }
}

#[derive(Clone, Copy)]
pub struct Descriptors<'a> {
    _owner: &'a PciRootBridgeIo,
    data: *const u8,
}

impl<'a> IntoIterator for Descriptors<'a> {
    type Item = Descriptor;

    type IntoIter = IterDescriptors<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IterDescriptors {
            descriptors: self,
            offset: 0,
        }
    }
}

#[derive(Debug)]
pub struct Descriptor {
    pub resource_type: DescriptorResourceType,
    pub general_flags: u8,
    pub type_specific_flags: u8,
    pub address_space_granularity: u64,
    pub address_range_minimum: u64,
    pub address_range_maximum: u64,
    pub address_translation_offset: u64,
    pub address_length: u64,
}

#[derive(Debug)]
pub enum DescriptorResourceType {
    MemoryRange,
    IORange,
    BusNumberRange,
}

pub struct IterDescriptors<'a> {
    descriptors: Descriptors<'a>,
    offset: usize,
}

impl<'a> Iterator for IterDescriptors<'a> {
    type Item = Descriptor;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            /* UEFI wants me to read 2B-aligned (even) addresses, so I have to read 2 bytes
            at a time.

            Because x86_64 is little endian, the most significant byte of `header` is
            *later* in the structure.
            */
            let header: u16 = *(self.descriptors.data.add(self.offset) as *const u16);
            match header & 0xff {
                0x8a => {
                    let size: u8 = (header >> 8) as u8;
                    assert!(size == 0x2b, "expected 0x2b, got {:#x}", size);

                    let nothing_and_resource_type: u16 =
                        *(self.descriptors.data.add(self.offset + 0x2) as *const u16);
                    assert!(
                        nothing_and_resource_type & 0xff == 0,
                        "expected 0x0, got {:#x}",
                        nothing_and_resource_type & 0xff
                    );

                    let resource_type: u8 = (nothing_and_resource_type >> 8) as u8;

                    let general_flags_and_type_specific_flags: u16 =
                        *(self.descriptors.data.add(self.offset + 0x4) as *const u16);
                    let general_flags: u8 = (general_flags_and_type_specific_flags & 0xff) as u8;
                    let type_specific_flags: u8 =
                        (general_flags_and_type_specific_flags >> 8) as u8;

                    /*
                    For some reason I'm only allowed to do aligned reads of u64.
                    The offsets of these 64-bit fields aren't 8B aligned, so I have
                    to read the components in smaller chunks.
                    */
                    let read_u64 = |offset: usize| {
                        let byte_0: u8 = *self.descriptors.data.add(self.offset + offset);
                        let byte_1: u8 = *self.descriptors.data.add(self.offset + offset + 1);
                        let byte_2: u8 = *self.descriptors.data.add(self.offset + offset + 2);
                        let byte_3: u8 = *self.descriptors.data.add(self.offset + offset + 3);
                        let byte_4: u8 = *self.descriptors.data.add(self.offset + offset + 4);
                        let byte_5: u8 = *self.descriptors.data.add(self.offset + offset + 5);
                        let byte_6: u8 = *self.descriptors.data.add(self.offset + offset + 6);
                        let byte_7: u8 = *self.descriptors.data.add(self.offset + offset + 7);
                        u64::from_le_bytes([
                            byte_0, byte_1, byte_2, byte_3, byte_4, byte_5, byte_6, byte_7,
                        ])
                    };

                    let address_space_granularity = read_u64(0x6);

                    let address_range_minimum: u64 = read_u64(0xe);
                    let address_range_maximum: u64 = read_u64(0x16);
                    let address_translation_offset: u64 = read_u64(0x1e);
                    let address_length: u64 = read_u64(0x26);

                    self.offset += 3 + size as usize;
                    Some(Descriptor {
                        resource_type: match resource_type {
                            0 => DescriptorResourceType::MemoryRange,
                            1 => DescriptorResourceType::IORange,
                            2 => DescriptorResourceType::BusNumberRange,
                            _ => unreachable!(),
                        },
                        general_flags,
                        type_specific_flags,
                        address_space_granularity,
                        address_range_minimum,
                        address_range_maximum,
                        address_translation_offset,
                        address_length,
                    })
                }
                0x79 => {
                    assert!(header >> 8 == 0);

                    None
                }
                tag => panic!("invalid descriptor tag: {:#x}", tag),
            }
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
#[unsafe_protocol(PciIoProtocol::GUID)]
pub struct PciIo(PciIoProtocol);
