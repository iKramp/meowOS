#!/usr/bin/env bash

#mkdir -p iso/EFI/BOOT
#cp kernel_build_files/kernel.bin iso/boot/
#cp grub.cfg iso/boot/grub/
#setup filesystem

LIMINE_DATA_DIR=$(limine --print-datadir)

if [ -z "$LIMINE_DATA_DIR" ]; then
    echo "Error: Failed to retrieve Limine data directory."
    exit 1
fi

mkdir -p iso/EFI/BOOT
cp -u $LIMINE_DATA_DIR/limine-uefi-cd.bin iso/
cp -u $LIMINE_DATA_DIR/limine-bios-cd.bin iso/
cp -u $LIMINE_DATA_DIR/limine-bios.sys iso/
cp -u assets/limine.conf iso/
cp -u kernel_build_files/kernel.bin iso/EFI/BOOT/

xorriso -as mkisofs -b "/limine-bios-cd.bin" \
        -no-emul-boot -boot-load-size 4 -boot-info-table \
        --efi-boot "/limine-uefi-cd.bin" \
        -efi-boot-part --efi-boot-image --protective-msdos-label \
        "iso/" -o kernel_build_files/image.iso

limine bios-install kernel_build_files/image.iso
