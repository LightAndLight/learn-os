# Variables

## Build directory
BUILD := .build

## Optimisation level
OPT := debug


# Top-level targets

.PHONY: run
run : $(BUILD)/bootloader.efi.$(OPT) $(BUILD)/kernel.bin.$(OPT)
	mkdir -p $(BUILD)/esp/efi/boot
	cp $(BUILD)/bootloader.efi.$(OPT) $(BUILD)/esp/efi/boot/bootx64.efi
	cp $(BUILD)/kernel.bin.$(OPT) $(BUILD)/esp/kernel.bin

	cp $$OVMF_PATH/OVMF_CODE.fd $(BUILD)
	cp $$OVMF_PATH/OVMF_VARS.fd $(BUILD)
	chmod 0666 $(BUILD)/OVMF_CODE.fd $(BUILD)/OVMF_VARS.fd

	# Debugging:
	# * `-no-reboot`: don't reboot on error (I find it easier to debug things this way)
	# * `-d int`: print interrupts to stdout
	# * `-D qemu.log`: write debug into to `qemu.log` instead of stderr (need to keep the terminal clean for serial communication over stdio)
	#
	# Machine:
	# * `-M pc-i440fx-8.2`: 440FX northbridge + PIIX3 southbridge (see also [QEMU i440FX PC](https://www.qemu.org/docs/master/system/i386/pc.html))
	#
	# Output:
	# * `-chardev stdio,id=chr0`: make stdin and stdout available to the VM
	# * `-nographic`: run QEMU without GUI
	#
	# Devices:
	# * `-nic none`: disable network card
	# * `-usb`: enable the USB host controller in the PIIX3 (see also: [QEMU USB Emulation](https://www.qemu.org/docs/master/system/devices/usb.html))
	# * `-device usb-kbd`: receive keyboard input via USB
	# * `-device pci-serial,chardev=chr0`: [add a PCI serial device](https://www.qemu.org/docs/master/specs/pci-serial.html)
	#   and connect it to stdin / stdout via the previously-defined character device.
	#
	# Note: `-enable-kvm`, which is a commonly suggested QEMU flag, is not compatible with with `-d int`.
	qemu-system-x86_64 \
		-no-reboot \
		-d int \
		-D qemu.log \
		-M pc-i440fx-8.2 \
		-chardev stdio,id=chr0 \
		-display none \
		-nic none \
		-device pci-serial,chardev=chr0 \
		-drive if=pflash,format=raw,readonly=on,file=$(BUILD)/OVMF_CODE.fd \
		-drive if=pflash,format=raw,readonly=off,file=$(BUILD)/OVMF_VARS.fd \
		-drive format=raw,file=fat:rw:$(BUILD)/esp

.PHONY: debug
debug : bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.efi $(BUILD)/kernel.bin.$(OPT)
	mkdir -p $(BUILD)/esp/efi/boot
	cp bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.efi $(BUILD)/esp/efi/boot/bootx64.efi
	cp $(BUILD)/kernel.bin.$(OPT) $(BUILD)/esp/kernel.bin

	cp $$OVMF_PATH/OVMF_CODE.fd $(BUILD)
	cp $$OVMF_PATH/OVMF_VARS.fd $(BUILD)
	chmod 0666 $(BUILD)/OVMF_CODE.fd $(BUILD)/OVMF_VARS.fd

	$(TERM) -e \
		qemu-system-x86_64 \
			-s \
			-no-reboot \
			-d int \
			-D qemu.log \
			-M pc-i440fx-8.2 \
			-chardev stdio,id=chr0 \
			-display none \
			-nic none \
			-device pci-serial,chardev=chr0 \
			-drive if=pflash,format=raw,readonly=on,file=$(BUILD)/OVMF_CODE.fd \
			-drive if=pflash,format=raw,readonly=off,file=$(BUILD)/OVMF_VARS.fd \
			-drive format=raw,file=fat:rw:$(BUILD)/esp &
	lldb \
		-O "settings set target.x86-disassembly-flavor intel" \
		-O "target create --no-dependents --arch x86_64 bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.efi --symfile bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.pdb" \
		-O "gdb-remote localhost:1234"

.PHONY: build
build : $(BUILD)/bootloader.efi.$(OPT) $(BUILD)/kernel.bin.$(OPT)

.PHONY: clean
clean :
	rm -rf bootloader/target kernel/target $(BUILD)


# Bootloader

bootloader_build_flags := --target x86_64-unknown-uefi
ifeq ($(OPT),release)
	bootloader_build_flags += --release
endif

bootloader_build_deps := bootloader/Cargo.toml
bootloader_build_deps += $(shell fd -e rs --full-path bootloader/src)
bootloader_build_deps += $(shell fd -e rs --full-path common/src)
bootloader_build_deps += $(shell fd -e rs --full-path uefi-pci/src)
bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.efi : $(bootloader_build_deps)
	cd bootloader && cargo build $(bootloader_build_flags)

bootloader_debug_deps := $(bootloader_build_deps) bootloader/x86_64-unknown-uefi-debug.json
bootloader/target/x86_64-unknown-uefi-debug/debug/bootloader.efi : $(bootloader_debug_deps) 
	cd bootloader && cargo build -Z build-std --target x86_64-unknown-uefi-debug.json

$(BUILD)/bootloader.efi.$(OPT) : bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.efi
	mkdir -p $(BUILD)
	cp $< $@


# Kernel

kernel_build_target := x86_64-unknown-kernel
kernel_build_target_file := $(kernel_build_target).json
kernel_build_flags := --target $(kernel_build_target_file) -Z build-std=core
ifeq ($(OPT),release)
	kernel_build_flags += --release
endif

kernel_build_deps := kernel/Cargo.toml
kernel_build_deps += $(shell fd -e rs --full-path kernel/src)
kernel_build_deps += $(shell fd -e rs --full-path common/src)
kernel_build_deps += kernel/$(kernel_build_target_file) kernel/kernel.ld
kernel/target/$(kernel_build_target)/$(OPT)/kernel : $(kernel_build_deps)
	cd kernel && cargo build $(kernel_build_flags)

$(BUILD)/kernel.elf.$(OPT) : kernel/target/$(kernel_build_target)/$(OPT)/kernel
	mkdir -p $(BUILD)
	cp $< $@

$(BUILD)/kernel.bin.$(OPT) : $(BUILD)/kernel.elf.$(OPT)
	objcopy \
		-O binary \
		--dump-section .header=$(BUILD)/header.bin.$(OPT) \
		--dump-section .code=$(BUILD)/code.bin.$(OPT) \
		--dump-section .rodata=$(BUILD)/rodata.bin.$(OPT) \
		--dump-section .rwdata=$(BUILD)/rwdata.bin.$(OPT) \
		$< \
		/dev/null
	cat \
		$(BUILD)/header.bin.$(OPT) \
		$(BUILD)/code.bin.$(OPT) \
		$(BUILD)/rodata.bin.$(OPT) \
		$(BUILD)/rwdata.bin.$(OPT) \
		> $@
