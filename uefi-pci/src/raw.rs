use uefi::{
    data_types::PhysicalAddress,
    table::boot::{AllocateType, MemoryType},
    Handle,
};
use uefi_raw::{guid, Guid, Status};

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
    ) -> Status,
    pub poll_io: unsafe extern "efiapi" fn(
        *const Self,
        PciRootBridgeIoProtocolWidth,
        u64,
        u64,
        u64,
        u64,
        *mut u64,
    ) -> Status,
    pub mem: PciRootBridgeIoProtocolAccess,
    pub io: PciRootBridgeIoProtocolAccess,
    pub pci: PciRootBridgeIoProtocolAccess,
    pub copy_mem: unsafe extern "efiapi" fn(
        *const Self,
        PciRootBridgeIoProtocolWidth,
        u64,
        u64,
        usize,
    ) -> Status,
    pub map: unsafe extern "efiapi" fn(
        *const Self,
        PciRootBridgeIoProtocolOperation,
        *const u8,
        *mut usize,
        *mut PhysicalAddress,
        *mut *const u8,
    ) -> Status,
    pub unmap: unsafe extern "efiapi" fn(*const Self, *const u8) -> Status,
    pub allocate_buffer: unsafe extern "efiapi" fn(
        *const Self,
        AllocateType,
        MemoryType,
        usize,
        *mut *const u8,
        u64,
    ) -> Status,
    pub free_buffer: unsafe extern "efiapi" fn(*const Self, usize, *const u8) -> Status,
    pub flush: unsafe extern "efiapi" fn(*const Self) -> Status,
    pub get_attributes: unsafe extern "efiapi" fn(*const Self, *mut u64, *mut u64) -> Status,
    pub set_attributes: unsafe extern "efiapi" fn(*const Self, u64, *mut u64, *mut u64) -> Status,
    pub configuration: unsafe extern "efiapi" fn(*const Self, *mut *const u8) -> Status,
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
    pub read: unsafe extern "efiapi" fn(
        *const PciRootBridgeIoProtocol,
        PciRootBridgeIoProtocolWidth,
        u64,
        usize,
        *mut u8,
    ) -> Status,
    pub write: unsafe extern "efiapi" fn(
        *const PciRootBridgeIoProtocol,
        PciRootBridgeIoProtocolWidth,
        u64,
        usize,
        *mut u8,
    ) -> Status,
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

#[repr(C)]
#[derive(Debug)]
pub struct PciIoProtocol {
    pub poll_mem: unsafe extern "efiapi" fn(
        *const Self,
        PciIoProtocolWidth,
        u8,
        u64,
        u64,
        u64,
        u64,
        *mut u64,
    ) -> Status,
    pub poll_io: unsafe extern "efiapi" fn(
        *const Self,
        PciIoProtocolWidth,
        u8,
        u64,
        u64,
        u64,
        u64,
        *mut u64,
    ) -> Status,
    pub mem: PciIoProtocolAccess,
    pub io: PciIoProtocolAccess,
    pub pci: PciIoProtocolConfigAccess,
    pub copy_mem: unsafe extern "efiapi" fn(
        *const Self,
        PciIoProtocolWidth,
        u8,
        u64,
        u8,
        u64,
        usize,
    ) -> Status,
    pub map: unsafe extern "efiapi" fn(
        *const Self,
        PciIoProtocolOperation,
        *const u8,
        *mut usize,
        *mut PhysicalAddress,
        *mut *const u8,
    ) -> Status,
    pub unmap: unsafe extern "efiapi" fn(*const Self, *const u8) -> Status,
    pub allocate_buffer: unsafe extern "efiapi" fn(
        *const Self,
        AllocateType,
        MemoryType,
        usize,
        *mut *const u8,
        u64,
    ) -> Status,
    pub free_buffer: unsafe extern "efiapi" fn(*const Self, usize, *const u8) -> Status,
    pub flush: unsafe extern "efiapi" fn(*const Self) -> Status,
    pub get_location: unsafe extern "efiapi" fn(
        *const Self,
        *mut usize,
        *mut usize,
        *mut usize,
        *mut usize,
    ) -> Status,
    pub attributes: unsafe extern "efiapi" fn(
        *const Self,
        PciIoProtocolAttributeOperation,
        u64,
        *mut u64,
    ) -> Status,
    pub get_bar_attributes: unsafe extern "efiapi" fn(*const Self, u8, *mut u64, *mut u8) -> Status,
    pub set_bar_attributes:
        unsafe extern "efiapi" fn(*const Self, u64, u8, *mut u64, *mut u64) -> Status,
}

impl PciIoProtocol {
    pub const GUID: Guid = guid!("4cf5b200-68b8-4ca5-9eec-b23e3f50029a");
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciIoProtocolWidth {
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
pub struct PciIoProtocolAccess {
    pub read: unsafe extern "efiapi" fn(
        *const PciIoProtocol,
        PciIoProtocolWidth,
        u8,
        u64,
        usize,
        *mut u8,
    ) -> Status,
    pub write: unsafe extern "efiapi" fn(
        *const PciIoProtocol,
        PciIoProtocolWidth,
        u8,
        u64,
        usize,
        *mut u8,
    ) -> Status,
}

#[repr(C)]
#[derive(Debug)]
pub struct PciIoProtocolConfigAccess {
    pub read: unsafe extern "efiapi" fn(
        *const PciIoProtocol,
        PciIoProtocolWidth,
        u64,
        usize,
        *mut u8,
    ) -> Status,
    pub write: unsafe extern "efiapi" fn(
        *const PciIoProtocol,
        PciIoProtocolWidth,
        u64,
        usize,
        *mut u8,
    ) -> Status,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciIoProtocolOperation {
    BusMasterRead,
    BusMasterWrite,
    BusMasterCommonBuffer,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciIoProtocolAttributeOperation {
    Get,
    Set,
    Enable,
    Disable,
    Supported,
}
