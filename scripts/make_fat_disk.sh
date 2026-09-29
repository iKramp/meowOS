#!/usr/bin/env bash
set -e

INPUT_DIR=fat_disk
OUTPUT_IMG=assets/fat_disk.img
SIZE="${3:-64M}"

truncate -s "$SIZE" "$OUTPUT_IMG"

parted -s "$OUTPUT_IMG" mklabel gpt
parted -s "$OUTPUT_IMG" mkpart primary fat32 1MiB 100%

sgdisk --partition-guid=1:e9a75ddc-0587-45af-963f-ebbc44c99083 "$OUTPUT_IMG"

OFFSET=$((1024 * 1024))

# Format the partition in-place, without loop devices or mounting.
mformat -i "$OUTPUT_IMG@@$OFFSET" -F ::

# Copy files into the FAT filesystem.
mcopy -i "$OUTPUT_IMG@@$OFFSET" -s "$INPUT_DIR"/* ::/

FAT_UUID=$(blkid -p -s UUID -o value --offset "$OFFSET" "$OUTPUT_IMG")
PART_UUID=$(sgdisk -i 1 "$OUTPUT_IMG" | awk -F': ' '/Partition unique GUID/ {print $2}')

echo "FAT UUID:     $FAT_UUID"
echo "GPT PARTUUID: $PART_UUID"
