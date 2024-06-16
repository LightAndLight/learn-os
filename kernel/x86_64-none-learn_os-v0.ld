/* This linker script is used as part of the `x86_64-none-learn_os-v0.json` Rust target.
Cargo ends up producing an ELF executable, so there's an extra step required to get a
binary of the right format:

```
dir=$(mktemp --tempdir)

objcopy \
  -O binary \
  --dump-section .header=$tmpheader.bin \
  --dump-section .code=$tmp/code.bin \
  --dump-section .rodata=$tmp/rodata.bin \
  --dump-section .rwdata=$tmp/rwdata.bin \
  program.elf \
  /dev/null

cat \
  $dir/header.bin \
  $dir/code.bin \
  $dir/rodata.bin \
  $dir/rwdata.bin \
  > program.bin
```
*/

PHDRS {
  header PT_PHDR;
  code PT_LOAD FLAGS(5);   /* 0b101 - read + execute */
  rodata PT_LOAD FLAGS(4); /* 0b100 - read           */
  rwdata PT_LOAD FLAGS(6); /* 0b110 - read + write   */
}

SECTIONS {
  code_offset = SIZEOF(.header);
  code_size = SIZEOF(.code);
  code_addr = ADDR(.code);

  rodata_offset = code_offset + code_size;
  rodata_size = SIZEOF(.rodata);
  rodata_addr = ADDR(.rodata);

  rwdata_offset = rodata_offset + rodata_size;
  rwdata_size = SIZEOF(.rwdata);
  rwdata_addr = ADDR(.rwdata);

  .header : {
    /* magic bytes */
    BYTE(0x6c); BYTE(0x65); BYTE(0x61); BYTE(0x72); BYTE(0x6e); /* learn */
    BYTE(0x2d); /* - */
    BYTE(0x6f); BYTE(0x73); /* os */

    /* version */
    SHORT(0x0);

    /* code segment info */
    QUAD(code_offset); QUAD(code_size); QUAD(code_addr);

    /* rodata segment info */
    QUAD(rodata_offset); QUAD(rodata_size); QUAD(rodata_addr);

    /* rwdata segment info */
    QUAD(rwdata_offset); QUAD(rwdata_size); QUAD(rwdata_addr);
  } : header

  /* See Note [The kernel's entrypoint] */
  . = 0x1000;

  .code :
    ALIGN(0x1000)
    SUBALIGN(0) /* See Note [.text section alignment] */
  {
    *(.text.entrypoint)
    *(.text .text.*)
  } : code

  .rodata : ALIGN(0x1000) {
    *(.rodata .rodata.*)
  } : rodata

  .rwdata : ALIGN(0x1000) {
    *(.bss .bss.*)

    /*
    I didn't need it at the time, but including the .data sections here
    was a hack to get the rwdata section to have type `PROGBITS`. When only
    .bss are included, the rwdata section gets type `NOBITS`, so .bss isn't
    expanded in the binary.
    */
    *(.data .data.*)
  } : rwdata

  /DISCARD/ : {
	*(.comment*)
  }
}

/* Note [.text section alignment]

The .text sub-sections are 0-aligned to save space. This may cause inefficient
instruction cache usage.

A 64B cache line holds a 64B aligned region of memory.
A function that isn't 64B aligned will start part-way through a cache line;
the beginning of the cache line will have irrelevant code.
If that function were 64B long, then the end of the function would spill over into another cache line.
The entire function would require two memory fetches.
For an event more exaggerated example, imagine that the function starts on the byte before a 64B aligned address.
The function is guaranteed to spill over into the next cache line even if it's much shorter than 64B.

If the function was 64B aligned and it was 64B long, then the start of the function would be at the start of the cache line
and the whole function would fit into that single cache line.
Only one memory fetch required.
*/
