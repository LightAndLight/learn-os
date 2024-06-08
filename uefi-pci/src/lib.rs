#![no_std]

pub mod raw;

use raw::PciRootBridgeIoProtocol;
use uefi::proto::unsafe_protocol;

#[derive(Debug)]
#[repr(transparent)]
#[unsafe_protocol(PciRootBridgeIoProtocol::GUID)]
pub struct PciRootBridgeIo(PciRootBridgeIoProtocol);
