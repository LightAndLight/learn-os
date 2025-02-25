# Device management

I want some form of I/O as soon as possible. It's important that I can interact with my computer.
I also want to do I/O in ways that are applicable to my personal computer.
While I'm currently developing on QEMU, I think it would be cool to eventually port this project to hardware.

When the OSDev Wiki tells me 

* <https://stackoverflow.com/questions/14194798/is-there-a-specification-of-x86-i-o-port-assignment>
  * Here's the docs for my [chipset](https://en.wikipedia.org/wiki/Chipset), the [Z370](https://en.wikipedia.org/wiki/Platform_Controller_Hub#Cannon_Point):
    [Intel® 300 Series and Intel® C240 Series Chipset Family Platform Controller Hub, volume 1](https://web.archive.org/web/20191009110648/https://www.intel.com/content/dam/www/public/us/en/documents/datasheets/300-series-chipset-pch-datasheet-vol-1.pdf)
    * Chapter 4 contains memory mapping info:
      * PCI devices and functions
      * Fixed and variable I/O ports
      * Memory-mapped I/O ranges

* And docs for my processor
  * [8th and 9th Generation Intel® Core™ Processor Families and Intel® Xeon® E Processor Families (Datasheet, Volume 1 of 2)](https://web.archive.org/web/20231208130533/https://www.intel.com/content/www/us/en/content-details/337344/8th-and-9th-generation-intel-core-processor-families-and-intel-xeon-e-processor-families-datasheet-volume-1-of-2.html)
    * 2.2 gives an overview of the processor's PCIe interface
  * [8th and 9th Generation Intel® Core™ Processor Families and Intel® Xeon® E Processor Family Datasheet, Volume 2 of 2](https://web.archive.org/web/20240608225053/https://www.intel.com/content/www/us/en/content-details/337345/8th-and-9th-generation-intel-core-processor-families-and-intel-xeon-e-processor-family-datasheet-volume-2-of-2.html)
    * Ch 2 describes the system address map
      * PCIe configuration space needs to be memory-mapped in a 32bit address space.
        `TOLUD` register the lower bound of this region, and `TOLUD` masks all memory
        up to 4GB.
        PCIe configuration space starts at `PCIEXBAR` (section 3.17) which must be greater than `TOLUD`.
    * `TOLUD` is a host bridge register (section 3.37), and so is `PCIEXBAR`. The host bridge appears as
      a PCI device on PCI bus 0 (section 2.2).
      
      Isn't this a bootstrap problem? I can only set `TOLUD` and `PCIEXBAR` after I've memory-mapping the
      configuration space, but I need to set them *in order to* memory map the configuration space.

      I think I'm making a mistake by equivocating PCI and PCIe. They are not the same thing.
      The host bridge is a PCI device, and the PCI configuration space is accessed using
      the `CONFIG_ADDRESS` and `CONFIG_DATA` I/O ports, which are available on 9th gen core processors (section 2.17).
      Those I/O ports are used to set `TOLUD`, and then `PCIEXBAR` which memory-maps the PCIe configuration space.

      After this, the PCI configuration is accessible starting at `PCIEXBAR`, because PCIe is backwards compabile
      with PCI.

      [The Wikipedia article on PCI configuration space](https://web.archive.org/web/20240607230825/https://en.wikipedia.org/wiki/PCI_configuration_space#Software_implementation)
      is misleading, because it implies that the `CONFIG_ADDRESS`/`CONFIG_DATA` I/O port approach is mutually
      exclusive with the memory-mapped approach.

* QEMU options
  * ```
    $ qemu-system-x86_64 -M '?'

    Supported machines are:
    microvm              microvm (i386)
    pc                   Standard PC (i440FX + PIIX, 1996) (alias of pc-i440fx-8.2)
    pc-i440fx-8.2        Standard PC (i440FX + PIIX, 1996) (default)
    ...
    q35                  Standard PC (Q35 + ICH9, 2009) (alias of pc-q35-8.2)
    pc-q35-8.2           Standard PC (Q35 + ICH9, 2009)
    ...
    isapc                ISA-only PC
    none                 empty machine
    x-remote             Experimental remote machine
    ```

    i440FX and Q35 stand out as real-world options.

  * [QEMU's listing of i440FX specs](https://www.qemu.org/docs/master/system/i386/pc.html)

    [Intel 440FX on Wikipedia](https://en.wikipedia.org/wiki/Intel_440FX)

    [Intel 440 chipset family documentation](https://web.archive.org/web/20041127232037/https://www.intel.com/design/archives/chipsets/440/index.htm)

  * Q35

    [Intel 3 series express chipset family datasheet](https://web.archive.org/web/20080920211105/https://www.intel.com/Assets/PDF/datasheet/316966.pdf)

* [PCI configuration space on Wikipedia](https://en.wikipedia.org/wiki/PCI_configuration_space)

  [Intel PCIe guide](https://web.archive.org/web/20180927050219/http://www.csit-sun.pub.ro/~cpop/Documentatie_SMP/Intel_Microprocessor_Systems/Intel_ProcessorNew/Intel%20White%20Paper/Accessing%20PCI%20Express%20Configuration%20Registers%20Using%20Intel%20Chipsets.pdf)

  * According to this guide, PCI configuration via I/O ports 0xcf8 and 0xcfc is "legacy"

    * Section 3.1 of the 440X PMC datasheet
    * Section 4.5 in the Q35 datasheet

  * [Old PCI spec](https://ics.uci.edu/~harris/ics216/pci/PCI_22.pdf)

    Some evergreen specifications, such as device identification in 6.2.1
  
  * [PCI on Osdev Wiki](https://wiki.osdev.org/PCI#Common_Header_Fields)

  * Memory-mapped PCIe configuration

    * Z370

  * Probably just use [UEFI's PCI protocol](https://uefi.org/specs/UEFI/2.9_A/14_Protocols_PCI_Bus_Support.html) to get
    at the configuration

    * Start to query things using the PCIe interface
      * Ch 2 of the 440X PIIX datasheet - wrong, use PMC datasheet for the PCI root
      * Ch 6 of Q35 chipset datasheet
      * Ch 13 of 300 series chipset datasheet vol 2

    * UEFI PCI Root Bridge I/O interface is the starting point

* USB

  * Host controller is accessed through PCI
    * Section 3.6 (USB host controller), Section 2.4 (PCI configuration registers - USB), Section 2.8 (USB I/O Registers) in PIIX+PIIX3 datasheet

    * Looks like UEFI / BIOS sets it up nicely in QEMU (makes sense, since the USB keyboard works)

    * [Intel UHCI design guide](https://ftp.riken.jp/NetBSD/misc/blymn/uhci11d.pdf)

    * [USB 1.1 spec](https://fabiensanglard.net/usbcheat/usb1.1.pdf)

    * Far out, that's kind of a lot. Maybe leave it 'till later?

* PS/2 emulation

  * USB host controllers can emulate a PS/2 controller in ["USB legacy mode"](https://wiki.osdev.org/%228042%22_PS/2_Controller#USB_Legacy_Support) 

  * PIIX3 does have this (`LEGSUP` - datasheet section 2.4.14), where you can enable

  * Also kinda complex, I want to get shit done

* Serial

  * [QEMU PCI serial device](https://www.qemu.org/docs/master/specs/pci-serial.html)
  * [-chardev stdio](https://qemu.readthedocs.io/en/v7.2.9/system/invocation.html#hxtool-6)

    `-chardev stdio=char0 -serial chardev:char0`

    or just `-serial stdio`?

  * [PC16550D](https://web.archive.org/web/20180826215135/http://www.ti.com/lit/ds/symlink/pc16550d.pdf) UART

    * QEMU's PCI serial device will map the registers in 6.8.1 to the address in the BAR.
      It's an IO bar, so the registers are in I/O space.
