{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            rust-overlay.overlays.default
          ];
        };

        # rustVersion = "1.75.0";

      in {
        devShell =
          pkgs.mkShell {
            packages = with pkgs; [
              binutils

              # (rust-bin.stable.${rustVersion}.default.override {
              (rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
                extensions = [
                  "cargo"
                  "clippy"
                  "rustc"
                  "rust-src"
                  "rustfmt"
                  "rust-analyzer"
                ];

                targets = [
                  "x86_64-unknown-linux-gnu"
                  "x86_64-unknown-uefi"
                  "x86_64-unknown-none"
                ];
              # })
              }))


              qemu
              gdb
              lldb
            ];

            OVMF_PATH = "${pkgs.OVMF.fd}/FV";

            shellHook = ''
              export MANPATH="${pkgs.binutils.man}/share/man:$(manpath)"
            '';
          };
      }
    );
}
