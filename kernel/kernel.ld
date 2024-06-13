SECTIONS {
  /* See Note [The kernel's entrypoint] */
  . = 0x1000;

  .text : SUBALIGN(0) /* See Note [.text section alignment] */ {
    *(.text.entrypoint)
    *(.text .text.*)
  }

  .rodata : {
    *(.rodata .rodata.*)
  }

  /DISCARD/ : {
	*(.comment*)
  }
}

/* Note [.text section alignment]

Since we're building a flat binary, our text sections don't have any alignment requirements.
*/
