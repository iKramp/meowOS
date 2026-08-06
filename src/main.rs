use std::{fs::File, path::Path, process::Stdio};

#[allow(dead_code)]
enum RunMode {
    Kvm,
    Tcg,
    Record,
    Replay,
}
impl RunMode {
    pub fn is_rr(&self) -> bool {
        match self {
            Self::Kvm | Self::Tcg => false,
            Self::Replay | Self::Record => true,
        }
    }
}

fn main() {
    //chose whether to debug with GDB
    let debug = false;
    let uefi = false;
    let snapshot = true;
    let cores = 1;
    let net = false;
    let run_mode = RunMode::Kvm;

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    //general config
    cmd.arg("-debugcon").arg("stdio");
    cmd.arg("-d")
        .arg("cpu_reset,guest_errors,unimp")
        .arg("-D")
        .arg("./log.txt")
        .arg("-no-reboot")
        .arg("-no-shutdown");

    //memory
    cmd.arg("-m").arg("256M");

    //cpu
    match run_mode {
        RunMode::Kvm => {
            cmd.arg("-machine").arg("q35");
            cmd.arg("-cpu").arg("host,invtsc");
            cmd.arg("-enable-kvm");
        }
        RunMode::Tcg => {
            cmd.arg("-machine").arg("q35");
            cmd.arg("-accel").arg("tcg");
            cmd.arg("-cpu").arg("max");
        }
        RunMode::Record => {
            //delete overlays and kernel.rr
            if Path::new("assets/kernel.rr").exists() {
                std::fs::remove_file("assets/kernel.rr").expect("Failed to delete kernel.rr");
            }
            if Path::new("assets/rr_state.qcow2").exists() {
                std::fs::remove_file("assets/rr_state.qcow2").expect("Failed to delete rr_state.qcow2");
            }
            for overlay in ["assets/test_disk_rr_overlay.qcow2", "assets/fat_disk_rr_overlay.qcow2"] {
                if Path::new(overlay).exists() {
                    std::fs::remove_file(overlay).expect("Failed to delete overlay");
                }
            }

            cmd.arg("-machine").arg("q35");
            cmd.arg("-accel").arg("tcg");
            cmd.arg("-cpu").arg("max");

            cmd.arg("-icount")
                .arg("shift=auto,rr=record,rrfile=assets/kernel.rr,rrsnapshot=start");
        }
        RunMode::Replay => {
            cmd.arg("-machine").arg("q35");
            cmd.arg("-accel").arg("tcg");
            cmd.arg("-cpu").arg("max");

            cmd.arg("-icount")
                .arg("shift=auto,rr=replay,rrfile=assets/kernel.rr,rrsnapshot=start");
        }
    }

    cmd.arg("-smp").arg(cores.to_string());
    cmd.arg("-boot").arg("d");

    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
    }

    if run_mode.is_rr() {
        //empty for saving vm state
        cmd.arg("-drive").arg("file=assets/rr_state.qcow2,if=none,id=rr,format=qcow2");

        //create rr_state if it doesn't exist
        if !Path::new("assets/rr_state.qcow2").exists() {
            let status = std::process::Command::new("qemu-img")
                .args(["create", "-f", "qcow2", "assets/rr_state.qcow2", "128M"])
                .status()
                .expect("failed to create rr_state.qcow2");
            if !status.success() {
                panic!("failed to create rr_state.qcow2");
            }
        }
    }

    //kernel image
    cmd.arg("-drive")
        .arg("id=boot_cd,format=raw,file=kernel_build_files/image.iso,media=cdrom,if=none,read-only=on");
    if run_mode.is_rr() {
        cmd.arg("-drive")
            .arg("driver=blkreplay,if=none,image=boot_cd,id=boot_cd-blkreplay,read-only=on");
        cmd.arg("-device").arg("ide-cd,drive=boot_cd-blkreplay");
    } else {
        cmd.arg("-device").arg("ide-cd,drive=boot_cd,bus=ide.1");
    }

    //ahci disks
    cmd.arg("-device").arg("ahci,id=ahci");
    let mut disk_ids = Vec::new();

    let disks = [("test_disk", "assets/ahci_disk.img"), ("fat_disk", "assets/fat_disk.img")];

    for (id, image) in disks {
        if run_mode.is_rr() {
            let overlay = format!("assets/{}_rr_overlay.qcow2", id);

            create_overlay(&overlay, image);

            cmd.arg("-drive")
                .arg(format!("id={},file={},if=none,format=qcow2", id, overlay));
        } else {
            if snapshot {
                cmd.arg("-drive")
                    .arg(format!("id={},format=raw,file={},if=none,snapshot=on", id, image));
            } else {
                cmd.arg("-drive").arg(format!("id={},format=raw,file={},if=none", id, image));
            }
        }

        disk_ids.push(id);
    }

    assert!(disk_ids.len() < 6, "update code");
    for (i, name) in disk_ids.iter().enumerate() {
        if run_mode.is_rr() {
            let rr_disk = format!("driver=blkreplay,if=none,image={},id={}-blkreplay", name, name);
            let device = format!("ide-hd,drive={}-blkreplay,bus=ahci.{}", name, i);
            cmd.arg("-drive").arg(rr_disk);
            cmd.arg("-device").arg(device);
        } else {
            let device = format!("ide-hd,drive={},bus=ahci.{}", name, i);
            cmd.arg("-device").arg(device);
        }
    }

    if net {
        //networking
        cmd.arg("-netdev")
            .arg("tap,id=net0,ifname=tap0,script=no,downscript=no")
            .arg("-device")
            .arg("e1000e,netdev=net0")
            .arg("-object")
            .arg("filter-dump,id=f1,netdev=net0,file=packets.pcap");
    } else {
        cmd.arg("-net").arg("none");
    }

    //logging
    let serial_file = File::create("qemu_serial.log").expect("Failed to create log file");
    cmd.stdout(Stdio::from(serial_file));
    let qemu_log = File::create("qemu_stderr.log").expect("Failed to create log file");
    cmd.stderr(Stdio::from(qemu_log));

    //debug
    if debug {
        cmd.arg("-s");
        cmd.arg("-S");
    }

    println!(
        "Running QEMU with command: {} {}",
        cmd.get_program().to_str().unwrap(),
        cmd.get_args().map(|a| a.to_str().unwrap()).collect::<Vec<_>>().join(" ")
    );

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

fn create_overlay(name: &str, backing: &str) {
    let backing = std::fs::canonicalize(backing).expect("idk");

    if Path::new(name).exists() {
        return;
    }

    println!("creating overlay {} for {:?}", name, backing);

    let status = std::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            "-b",
            backing.to_str().expect("idk"),
            "-F",
            "raw",
            name,
        ])
        .status()
        .expect("failed to create overlay");

    assert!(status.success(), "failed creating overlay {}", name);
}
