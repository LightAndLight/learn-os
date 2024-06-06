#[macro_export]
macro_rules! wait_for_debugger {
    ($image_handle:expr, $system_table:expr) => {
        let image_base: u64 = {
            let loaded_image = $system_table
                .boot_services()
                .open_protocol_exclusive::<LoadedImage>($image_handle)
                .unwrap();
            let (image_base, _) = loaded_image.info();
            image_base as u64
        };

        info!("waiting for debugger... (image base = {:#x})", image_base);
        unsafe { asm!("2: jmp 2b") };
    };
}
