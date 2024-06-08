use uefi::{
    data_types::PhysicalAddress,
    table::boot::{AllocateType, MemoryType},
    Handle,
};
use uefi_raw::{guid, Guid};

#[repr(C)]
#[derive(Debug)]
pub struct PciRootBridgeIoProtocol {
    pub parent_handle: Handle,
    pub poll_mem: unsafe extern "efiapi" fn(
        *const Self,
        PciRootBridgeIoProtocolWidth,
        u64,
        u64,
        u64,
        u64,
        *mut u64,
    ),
    pub poll_io: unsafe extern "efiapi" fn(
        *const Self,
        PciRootBridgeIoProtocolWidth,
        u64,
        u64,
        u64,
        u64,
        *mut u64,
    ),
    pub mem: PciRootBridgeIoProtocolAccess,
    pub io: PciRootBridgeIoProtocolAccess,
    pub pci: PciRootBridgeIoProtocolAccess,
    pub copy_mem:
        unsafe extern "efiapi" fn(*const Self, PciRootBridgeIoProtocolWidth, u64, u64, usize),
    pub map: unsafe extern "efiapi" fn(
        *const Self,
        PciRootBridgeIoProtocolOperation,
        *const u8,
        *mut usize,
        *mut PhysicalAddress,
        *mut *const u8,
    ),
    pub unmap: unsafe extern "efiapi" fn(*const Self, *const u8),
    pub allocate_buffer: unsafe extern "efiapi" fn(
        *const Self,
        AllocateType,
        MemoryType,
        usize,
        *mut *const u8,
        u64,
    ),
    pub free_buffer: unsafe extern "efiapi" fn(*const Self, usize, *const u8),
    pub flush: unsafe extern "efiapi" fn(*const Self),
    pub get_attributes: unsafe extern "efiapi" fn(*const Self, *mut u64, *mut u64),
    pub set_attributes: unsafe extern "efiapi" fn(*const Self, u64, *mut u64, *mut u64),
    pub configuration: unsafe extern "efiapi" fn(*const Self, *mut *const u8),
    pub segment_number: u32,
}

impl PciRootBridgeIoProtocol {
    pub const GUID: Guid = guid!("2f707ebb-4a1a-11d4-9a38-0090273fc14d");
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciRootBridgeIoProtocolWidth {
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    FifoUint8,
    FifoUint16,
    FifoUint32,
    FifoUint64,
    FillUint8,
    FillUint16,
    FillUint32,
    FillUint64,
}

#[repr(C)]
#[derive(Debug)]
pub struct PciRootBridgeIoProtocolAccess {
    pub read:
        unsafe extern "efiapi" fn(*const Self, PciRootBridgeIoProtocolWidth, u64, usize, *mut u8),
    pub write:
        unsafe extern "efiapi" fn(*const Self, PciRootBridgeIoProtocolWidth, u64, usize, *mut u8),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciRootBridgeIoProtocolOperation {
    BusMasterRead,
    BusMasterWrite,
    BusMasterCommonBuffer,
    BusMasterRead64,
    BusMasterWrite64,
    BusMasterCommonBuffer64,
}
