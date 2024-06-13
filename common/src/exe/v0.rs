/// ASCII encoding of the letters `learn-os`.
pub const MAGIC_BYTES: [u8; 8] = [0x6c, 0x65, 0x61, 0x72, 0x6e, 0x2d, 0x6f, 0x73];
pub const VERSION: u16 = 0;

const VERSION_OFFSET: usize = 8;
const CODE_INFO_OFFSET: usize = 10;
const RODATA_INFO_OFFSET: usize = 34;
const RWDATA_INFO_OFFSET: usize = 58;

pub struct Exe<'a> {
    // Safety: the buffer must be at least `Header::ENCODED_SIZE` bytes long.
    buffer: &'a [u8],
}

impl<'a> Exe<'a> {
    /// Create an [`Exe`] view on a buffer, if the buffer is valid.
    pub fn parse(buffer: &'a [u8]) -> Result<Exe<'a>, Error> {
        if buffer.len() < Header::ENCODED_SIZE {
            return Err(Error::Length {
                minimum_length: Header::ENCODED_SIZE,
                actual_length: buffer.len(),
            });
        }

        let exe = Exe { buffer };

        if exe.magic_bytes() != MAGIC_BYTES {
            return Err(Error::MagicBytes {
                expected: MAGIC_BYTES,
                actual: exe.magic_bytes(),
            });
        }

        if exe.version() != VERSION {
            return Err(Error::Version {
                expected: 0,
                actual: exe.version(),
            });
        }

        Ok(exe)
    }

    /// Read the entire program header.
    pub fn header(&self) -> Header {
        Header {
            magic_bytes: self.magic_bytes(),
            version: self.version(),
            code_info: self.code_info(),
            rodata_info: self.rodata_info(),
            rwdata_info: self.rwdata_info(),
        }
    }

    /// Read the magic bytes from the program header.
    pub fn magic_bytes(&self) -> [u8; 8] {
        const SIZE: usize = 8;

        let slice = &self.buffer[0..SIZE];
        unsafe { *(slice.as_ptr() as *const [u8; SIZE]) }
    }

    /// Read the version from the program header.
    pub fn version(&self) -> u16 {
        const SIZE: usize = 2;

        let slice = &self.buffer[VERSION_OFFSET..(VERSION_OFFSET + SIZE)];
        unsafe { u16::from_le_bytes(*(slice.as_ptr() as *const [u8; SIZE])) }
    }

    /// Read the code segment info from the program header.
    pub fn code_info(&self) -> SegmentInfo {
        const SIZE: usize = SegmentInfo::ENCODED_SIZE;

        let slice = &self.buffer[CODE_INFO_OFFSET..(CODE_INFO_OFFSET + SIZE)];
        unsafe { SegmentInfo::from(*(slice.as_ptr() as *const [u8; SIZE])) }
    }

    /// The code segment.
    pub fn code(&self) -> &[u8] {
        let info = self.code_info();
        let start = info.start as usize;
        let size = info.size as usize;
        &self.buffer[start..(start + size)]
    }

    /// Read the rodata segment info from the program header.
    pub fn rodata_info(&self) -> SegmentInfo {
        const SIZE: usize = SegmentInfo::ENCODED_SIZE;

        let slice = &self.buffer[RODATA_INFO_OFFSET..(RODATA_INFO_OFFSET + SIZE)];
        unsafe { SegmentInfo::from(*(slice.as_ptr() as *const [u8; SIZE])) }
    }

    /// The rodata segment.
    pub fn rodata(&self) -> &[u8] {
        let info = self.rodata_info();
        let start = info.start as usize;
        let size = info.size as usize;
        &self.buffer[start..(start + size)]
    }

    /// Read the rwdata segment info from the program header.
    pub fn rwdata_info(&self) -> SegmentInfo {
        const SIZE: usize = SegmentInfo::ENCODED_SIZE;

        let slice = &self.buffer[RWDATA_INFO_OFFSET..(RWDATA_INFO_OFFSET + SIZE)];
        unsafe { SegmentInfo::from(*(slice.as_ptr() as *const [u8; SIZE])) }
    }

    /// The rwdata segment.
    pub fn rwdata(&self) -> &[u8] {
        let info = self.rwdata_info();
        let start = info.start as usize;
        let size = info.size as usize;
        &self.buffer[start..(start + size)]
    }
}

#[derive(Debug)]
pub enum Error {
    /// Buffer is smaller than the minimum possible executable size.
    Length {
        minimum_length: usize,
        actual_length: usize,
    },

    /// Executable's magic bytes are incorrect.
    MagicBytes { expected: [u8; 8], actual: [u8; 8] },

    /// Executable's version iis incorrect.
    Version { expected: u16, actual: u16 },
}

pub struct Header {
    /// Value is [`MAGIC_BYTES`].
    pub magic_bytes: [u8; 8],

    /// Value is [`VERSION`].
    pub version: u16,

    /** Code segment info.

    [`Info::load_address`] must also be the program's entrypoint.
    */
    pub code_info: SegmentInfo,

    /// Read-only data segment info.
    pub rodata_info: SegmentInfo,

    /// Read-write data segment info.
    pub rwdata_info: SegmentInfo,
}

impl Header {
    /** The size of a header on disk.

    `core::mem::size_of::<Header>()` may be different, because the struct's layout is
    controlled by the Rust compiler.
    */
    pub const ENCODED_SIZE: usize =
        // magic bytes
        8 +
        // version
        2 +
        // segment infos
        3 * SegmentInfo::ENCODED_SIZE;
}

pub struct SegmentInfo {
    /// Byte at which the segment begins, relative to the start of the binary.
    pub start: u64,

    /// Segment size, in bytes.
    pub size: u64,

    /** Virtual address at which the segment should be loaded.

    Must be 4KiB aligned.
    */
    pub load_address: u64,
}

impl SegmentInfo {
    /// The size of a segment info entry on disk.
    pub const ENCODED_SIZE: usize = 8 + 8 + 8;
}

impl From<[u8; 24]> for SegmentInfo {
    fn from(value: [u8; 24]) -> Self {
        // Safety: no array accesses exceed index 23.
        unsafe {
            SegmentInfo {
                start: u64::from_le_bytes(*(value[0..8].as_ptr() as *const [u8; 8])),
                size: u64::from_le_bytes(*(value[8..(8 + 8)].as_ptr() as *const [u8; 8])),
                load_address: u64::from_le_bytes(*(value[16..(16 + 8)].as_ptr() as *const [u8; 8])),
            }
        }
    }
}
