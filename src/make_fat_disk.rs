pub fn make_fat_disk() {
    let fat_disk_path = "assets/fat_disk.img";
    let fat_disk_dir = "fat_disk";

    let img_time = std::fs::metadata(fat_disk_path)
        .map(|meta| meta.modified().unwrap())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let dir_time = get_dir_modified_time(fat_disk_dir);

    if dir_time > img_time {
        println!("Creating FAT disk image because {:?} is newer than {:?}", dir_time, img_time);
        std::process::Command::new("sh")
            .args(["-c", "scripts/make_fat_disk.sh"])
            .status()
            .expect("Failed to create FAT disk image");
    } else {
        println!("FAT disk image is up to date.");
    }
}

fn get_dir_modified_time(path: &str) -> std::time::SystemTime {
    let mut latest_time = std::time::UNIX_EPOCH;

    for entry in std::fs::read_dir(path).expect("Failed to read directory") {
        let entry = entry.expect("Failed to read directory entry");
        let metadata = entry.metadata().expect("Failed to get metadata");

        if metadata.is_dir() {
            let dir_time = get_dir_modified_time(&entry.path().to_string_lossy());
            if dir_time > latest_time {
                latest_time = dir_time;
            }
        } else {
            let modified_time = metadata.modified().expect("Failed to get modified time");
            if modified_time > latest_time {
                latest_time = modified_time;
            }
        }
    }

    latest_time
}
