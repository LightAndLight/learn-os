# An executable format

Flat binaries have worked well for me so far.
I didn't want to cargo-cult something like [ELF](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format) because I didn't know how I'd benefit from the added complexity.
I've finally encountered the first stumbling block of flat binaries: global mutable variables.

When I tried to add a global mutable variable to my kernel, Rust put it in the [.bss](https://en.wikipedia.org/wiki/.bss) section.
I got a page fault for writing to a read-only page.
The `.bss` section is correctly linked into the binary by my linker script, but my whole kernel binary is mapped to read-only virtual memory so writing to the `.bss` "section" fails.

I *could* map the kernel to read-write memory, but I don't like that idea.
Since I'm playing so close to the metal, I like the idea of having guard rails to stop my code doing something really stupid.
I can use virtual memory to have the CPU enforce that the kernel's code and read-only data can't be unmodified.

This means I need to divide my code into two parts.
When setting up the kernel's page table, one part is mapped to a read+execute address space, and the other to read+write.
ELF calls these parts *segments*.

How do I tell the bootloader about the location, size, and permissions of each segment, so it can set up the correct page table?
I could inspect the kernel binary and then hard-code the values into the bootloader.
I'd have to recompile the bootloader every time I compile the kernel, and that's inconvenient.
Better to put this metadata into the executable itself in a well-known format.
Write the bootloader to assume that my kernel adheres to the format, and compile the bootloader once.
It will be able to load every version of my kernel that adheres to the executable format.
This sort of problem is why standardised executable formats like ELF exist.

Since I'm not concerned with compatibility, I've created my own simplified executable format for practise.

## The `learn-os` executable format (v0)

| File | Description |
| --- | --- |
| <../kernel/x86_64-none-learn_os-v0.json> | Rust cross-compilation target[^1] |
| <../kernel/x86_64-none-learn_os-v0.ld> | Linker script |
| <../common/src/exe/v0.rs> | Format spec |

A `learn-os` executable consists of a program header (little-endian) followed by three segments: code, rodata, and rwdata.
It's intended to be loaded into read-only memory, with the segments then copied into pages that are mapped
with the correct permissions. An executable that uses all 3 segment types will occupy a minimum of 12MiB of
RAM at runtime, because memory access permissions can't be set for regions smaller than a single page.

v0 is a compact executable format. It saves space at the expense of extra startup time (copying segments
to other pages). The alternative to copying segments is to memory map the executable directly. This
requires the segments to be page aligned within the executable so that the appropriate memory permissions
can be set. For small executables the page alignment means that most of the executable is padding.
As executables grow larger, the space savings from the compact format account for a smaller proportion
of the total size, and the time spent copying segments grows. Larger executables will benefit from
a mappable format, which can be specified in a new version.
