# Variables

## Build directory
BUILD := .build

## Optimisation level
OPT := debug


# Top-level targets

.PHONY: run
run : $(BUILD)/bootloader.efi $(BUILD)/kernel
	mkdir -p $(BUILD)/esp/efi/boot
	cp bootloader/target/x86_64-unknown-uefi/debug/bootloader.efi $(BUILD)/esp/efi/boot/bootx64.efi
	cp $(BUILD)/kernel $(BUILD)/esp/kernel.bin

	cp $$OVMF_PATH/OVMF_CODE.fd $(BUILD)
	cp $$OVMF_PATH/OVMF_VARS.fd $(BUILD)
	chmod 0666 $(BUILD)/OVMF_CODE.fd $(BUILD)/OVMF_VARS.fd

	# -enable-kvm is not compatible with -d int
	qemu-system-x86_64 \
		-no-reboot \
		-d int \
		-boot menu=on,splash-time=0 \
		-vga std \
		-drive if=pflash,format=raw,readonly=on,file=$(BUILD)/OVMF_CODE.fd \
		-drive if=pflash,format=raw,readonly=off,file=$(BUILD)/OVMF_VARS.fd \
		-drive format=raw,file=fat:rw:$(BUILD)/esp

.PHONY: debug
debug : bootloader/target/x86_64-unknown-uefi/debug/bootloader.efi $(BUILD)/kernel
	mkdir -p $(BUILD)/esp/efi/boot
	cp bootloader/target/x86_64-unknown-uefi/debug/bootloader.efi $(BUILD)/esp/efi/boot/bootx64.efi
	cp $(BUILD)/kernel $(BUILD)/esp/kernel.bin

	cp $$OVMF_PATH/OVMF_CODE.fd $(BUILD)
	cp $$OVMF_PATH/OVMF_VARS.fd $(BUILD)
	chmod 0666 $(BUILD)/OVMF_CODE.fd $(BUILD)/OVMF_VARS.fd
	
	qemu-system-x86_64 \
		-no-reboot \
		-s \
		-boot menu=on,splash-time=0 \
		-vga std \
		-drive if=pflash,format=raw,readonly=on,file=$(BUILD)/OVMF_CODE.fd \
		-drive if=pflash,format=raw,readonly=off,file=$(BUILD)/OVMF_VARS.fd \
		-drive format=raw,file=fat:rw:$(BUILD)/esp & \
	lldb \
		-O "target create --no-dependents --arch x86_64 bootloader/target/x86_64-unknown-uefi/debug/bootloader.efi --symfile bootloader/target/x86_64-unknown-uefi/debug/bootloader.pdb" \
		-O "gdb-remote localhost:1234"

.PHONY: build
build : $(BUILD)/bootloader.efi $(BUILD)/kernel

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
bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.efi : $(bootloader_build_deps)
	cd bootloader && cargo build $(bootloader_build_flags)

bootloader_debug_deps := $(bootloader_build_deps) bootloader/x86_64-unknown-uefi-debug.json
bootloader/target/x86_64-unknown-uefi-debug/debug/bootloader.efi : $(bootloader_debug_deps) 
	cd bootloader && cargo build -Z build-std --target x86_64-unknown-uefi-debug.json

$(BUILD)/bootloader.efi : bootloader/target/x86_64-unknown-uefi/$(OPT)/bootloader.efi
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

$(BUILD)/kernel : kernel/target/$(kernel_build_target)/$(OPT)/kernel
	mkdir -p $(BUILD)
	cp $< $@
