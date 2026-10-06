use std::path::PathBuf;
use std::process::Command;

const X86_TRAMPOLINE_ASM: &str = "src/arch/x86_64/multiprocessing/trampoline.asm";
const X86_PROBE_ASM: &str = "src/arch/x86_64/memory/probe.asm";

fn main() {
    let target = std::env::var("TARGET").expect("TARGET variable not set");

    println!("cargo:rerun-if-changed=linker_script.ld");

    if target.contains("x86_64") {
        println!("cargo:rerun-if-changed={}", X86_TRAMPOLINE_ASM);
        println!("cargo:rerun-if-changed={}", X86_PROBE_ASM);
    } else {
        panic!("Unsupported target architecture: {}", target);
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR variable not set");
    let link_script_file =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR variable not set")).join("linker_script.ld");

    if !(Command::new("nasm")
        .args(["-f", "elf64", X86_TRAMPOLINE_ASM, "-o", &(out_dir.clone() + "/trampoline.o")])
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
        .args(["-f", "elf64", X86_PROBE_ASM, "-o", &(out_dir.clone() + "/probe.o")])
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

    // Re-run the build script if the build configuration changes
    println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rustc-link-lib=static=trampoline");
    println!("cargo:rustc-link-lib=static=probe");
}
