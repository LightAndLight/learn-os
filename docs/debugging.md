# Debugging

## Bootloader

1. Insert a call to `wait_for_debugger!(image_handle, system_table)` somewhere that boot services are available

2. `make debug`

   Starts `qemu` in the background and `lldb` in the foreground, and runs some boilerplate
   `lldb` commands.

3. In `lldb`, run `c` (short for `continue`)

   LLDB pauses after it connects to `qemu`, so you have to kick things off manually.

4. Wait for the `qemu` system to display `waiting for debugger... (image base = <image base>)`
   and then hang

   Note the image base address (`<image base>`). It's used in a later step.

5. In `lldb`, run `process interrupt`

   This pauses execution of the bootloader.

6. In `lldb`, run `target modules load --file bootloader.efi .text <image base + 0x1000>`

   `0x1000` is the offset of the code relative to the beginning of the file.
   To verify this offset, run `objdump -x bootloader/target/x86_64-unknown-uefi/debug/bootloader.efi | sed -n '/BaseOfCode/p'`.
   When I wrote this, the output was `BaseOfCode		0000000000001000`.

   `<image base>` is in hexadecimal, so remember to do hexadecimal arithmetic!

7. Run `f` (short for `frame select`)

   If debug symbols are working correctly, you should see output like this (perhaps with different line numbers):

   ```
      36  	        };
      37  	
      38  	        info!("waiting for debugger... (image base = {:#x})", image_base);
   -> 39  	        unsafe { asm!("2: jmp 2b") };
      40  	    };
      41  	}
      42
   ```

   `wait_for_debugger` waits by running an infinite loop.

8. Use `j <line number>` (short for `jump <line number>`) to jump to the line after the one pointed to by `->`.

   In the above example, it would be `j 40`.

9. Do debugging things.

   Running `continue` will resume the bootloader from after the `wait_for_debugger` call.

## References

* <https://old.reddit.com/r/osdev/comments/144gojm/help_debugging_uefi_application_with_gdb_in_vs/>
