#!/usr/bin/env bash
set -e

INPUT_DIR=fat_disk
OUTPUT_IMG=assets/fat_disk.img
SIZE="${3:-512M}"

if [ -z "$INPUT_DIR" ] || [ -z "$OUTPUT_IMG" ]; then
    echo "Usage: $0 <input_folder> <output.img> [size]"
    exit 1
fi

truncate -s "$SIZE" "$OUTPUT_IMG"

parted -s "$OUTPUT_IMG" mklabel gpt
parted -s "$OUTPUT_IMG" mkpart primary fat32 1MiB 100%

LOOP=$(losetup --find --show -P "$OUTPUT_IMG")

mkfs.vfat -F 32 "${LOOP}p1" >/dev/null

MNT=$(mktemp -d)
mount "${LOOP}p1" "$MNT"

cp -r "$INPUT_DIR"/. "$MNT"/

sync

# print FAT filesystem UUID
FAT_UUID=$(blkid -s UUID -o value "${LOOP}p1")

# print GPT partition UUID (PARTUUID)
PART_UUID=$(lsblk -no PARTUUID "${LOOP}p1")

echo "FAT UUID:     $FAT_UUID"
echo "GPT PARTUUID: $PART_UUID"

umount "$MNT"
losetup -d "$LOOP"
rmdir "$MNT"
