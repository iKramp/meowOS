{
  description = "Rust OS Kernel Development Flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }: 
  let
    system = "x86_64-linux";
    overlays = [ (import rust-overlay) ];
    pkgs = import nixpkgs { inherit system overlays; };

    rust = pkgs.rust-bin.nightly."2025-03-03".default.override {
      targets = [ "x86_64-unknown-none" ];
      extensions = [ "rust-src" ];
    };

  in {
    devShells.${system}.default = pkgs.mkShell {

      buildInputs = [
        (pkgs.limine.override {
          enableAll = true;
        })
        rust
        pkgs.qemu
        pkgs.gdb
        pkgs.nasm
        pkgs.rust-analyzer
        pkgs.clippy
        pkgs.xorriso
        pkgs.cargo-expand
      ];

      shellHook = ''
        export RUST_STORE_PATH=${rust}
        exec zsh
      '';
    };
  };
}

