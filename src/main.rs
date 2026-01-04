use std::{fs::File, process::Stdio};

fn main() {
    //demangle the kernel.map file
    //let _ = std::process::Command::new("rustfilt")
    //    .arg("-i")
    //    .arg("/home/nejc/dev/meowOS/kernel.map")
    //    .arg("-o")
    //    .arg("/home/nejc/dev/meowOS/kernel.map");
    //

    //chose whether to debug with GDB
    let debug = true;
    let uefi = false;
    let snapshot = true;
    let cores = 1;

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    //general config
    cmd.arg("-debugcon").arg("stdio");
    cmd.arg("-d")
        .arg("cpu_reset")
        .arg("-D")
        .arg("./log.txt")
        .arg("-no-reboot");

    //cpu
    cmd.arg("-machine").arg("q35");
    cmd.arg("-cpu").arg("host,invtsc");
    cmd.arg("-enable-kvm");
    cmd.arg("-smp").arg(cores.to_string());

    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
    }


    //kernel image
    cmd.arg("-drive").arg("id=boot_cd,format=raw,file=kernel_build_files/image.iso,media=cdrom");
    cmd.arg("-boot").arg("order=d");

    //ahci disk
    if snapshot {
        cmd.arg("-drive")
            .arg("id=test_disk,format=raw,file=assets/ahci_disk.img,if=none,snapshot=on");
    } else {
        cmd.arg("-drive")
            .arg("id=test_disk,format=raw,file=assets/ahci_disk.img,if=none");
    }
    cmd.arg("-device").arg("ahci,id=ahci");
    cmd.arg("-device").arg("ide-hd,drive=test_disk,bus=ahci.0");

    //networking
    cmd.arg("-netdev")
        .arg("tap,id=net0,ifname=tap0,script=no,downscript=no")
        .arg("-device").arg("e1000e,netdev=net0");

    //logging
    let log_file = File::create("qemu_serial.log").expect("Failed to create log file");
    cmd.stdout(Stdio::from(log_file));

    //debug
    if debug {
        cmd.arg("-s");
        cmd.arg("-S");
    }

    let mut child = cmd.spawn().expect("Failed to start QEMU");

    if debug {
        let _ = std::process::Command::new("kitty")
            .arg("gdb")
            .arg("-x")
            .arg("~/dev/meowOS/assets/gdb_commands.txt")
            .spawn()
            .expect("Failed to start GDB")
            .wait()
            .expect("Failed to wait for GDB");
    }

    child.wait().expect("Failed to wait on QEMU process");
}

#[test]
fn test_run() {
    main();
}
