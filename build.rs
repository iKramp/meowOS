use std::{path::PathBuf, process::Command};

fn main() {
    // set by cargo, build scripts should use this directory for output files
    let _out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR variable not set"));
    let kernel = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel").expect("CARGO_BIN_FILE_KERNEL_kernel variable not set"));
    let cargo_manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR variable not set");
    let kernel_build_files_dir = PathBuf::from(cargo_manifest_dir.clone()).join("kernel_build_files");
    if !kernel_build_files_dir.exists() {
        std::fs::create_dir(kernel_build_files_dir.clone()).expect("Failed to create kernel_build_files directory");
    }
    std::fs::copy(kernel, kernel_build_files_dir.join("kernel.elf")).expect("Failed to copy kernel binary to kernel_build_files directory");
    let status = Command::new("objcopy")
        .arg("kernel_build_files/kernel.elf")
        .arg("kernel_build_files/kernel.bin")
        .current_dir(&cargo_manifest_dir)
        .status()
        .expect("Failed to run objcopy on kernel.elf");
    assert!(status.success());

    let status = Command::new(cargo_manifest_dir + "/scripts/make_disk.sh")
        .status()
        .expect("Failed to run make_disk.sh");
    assert!(status.success());

    println!("cargo:rerun-if-canged=build.rs");
}
