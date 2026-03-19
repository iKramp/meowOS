use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR variable not set");
    let link_script_file =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR variable not set")).join("linker_script.ld");

    if !(Command::new("nasm")
        .args([
            "-f",
            "elf64",
            "src/acpi/smp/trampoline.asm",
            "-o",
            &(out_dir.clone() + "/trampoline.o"),
        ])
        .status()
        .expect("Failed to run nasm on trampoline.asm")
        .success()
        && Command::new("ar")
            .arg("rcs")
            .arg(&(out_dir.clone() + "/libtrampoline.a"))
            .arg(&(out_dir.clone() + "/trampoline.o"))
            .status()
            .expect("Failed to run ar on trampoline.o")
            .success())
    {
        panic!("Failed to assemble trampoline.asm");
    }

    if !(Command::new("nasm")
        .args(["-f", "elf64", "src/memory/probe.asm", "-o", &(out_dir.clone() + "/probe.o")])
        .status()
        .expect("Failed to run nasm on probe.asm")
        .success()
        && Command::new("ar")
            .arg("rcs")
            .arg(&(out_dir.clone() + "/libprobe.a"))
            .arg(&(out_dir.clone() + "/probe.o"))
            .status()
            .expect("Failed to run ar on probe.o")
            .success())
    {
        panic!("Failed to assemble probe.asm");
    }

    // Set the flag to generate the linker map file
    println!("cargo:rustc-link-arg=-T{}", link_script_file.display());
    //println!("cargo:rustc-link-arg=Map=/home/nejc/dev/meowOS/kernel.map");

    // Re-run the build script if the build configuration changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=kernel/linker_script.ld");
    println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rustc-link-lib=static=trampoline");
    println!("cargo:rustc-link-lib=static=probe");
}
