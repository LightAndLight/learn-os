ENTRY(kernel)

SECTIONS {
  . = 0x1000; /* See Note [The kernel's virtual address] */
  
  .text : { *(.text*) }
  .rodata : { *(.rodata*) }
  /DISCARD/ : {
	*(.comment*)
  }
}
