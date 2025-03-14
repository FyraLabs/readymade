#!/bin/bash

CHROOT_DIR="/mnt/custom"
: ${DIVE_EXEC:="dive"}

# $1 will be the loop device path

usage() {
    echo "Usage: $0 <loop device path>"
    exit 1
}

if [ -z "$1" ]; then
    usage
fi

if [ ! -b "$1" ]; then
    echo "Error: $1 is not a block device"
    usage
fi

DISK="$1"

# Check if the chroot directory exists

is_luks() {
    if sudo cryptsetup isLuks "$1"; then
        return 0
    else
        return 1
    fi
}

EFI_PART="${DISK}p1"
XBOOT_PART="${DISK}p2"
ROOT_PART="${DISK}p3"
ROOT_DEV="$ROOT_PART"
# ROOT_PART="${DISK}p3"

if is_luks "$ROOT_PART"; then
    echo "Unlocking the root partition"
    sudo cryptsetup open "$ROOT_PART" root
    export ROOT_DEV="/dev/mapper/root"
else
    export ROOT_DEV="$ROOT_DISK"
fi

if [ ! -d "$CHROOT_DIR" ]; then
    echo "Creating the chroot directory"
    sudo mkdir -p "$CHROOT_DIR"
fi

echo "Mounting the root partition"
sudo mount "$ROOT_DEV" "$CHROOT_DIR"

echo "Mounting the boot partition"
sudo mount "$XBOOT_PART" "$CHROOT_DIR/boot"

echo "Mounting the EFI partition"
sudo mount "$EFI_PART" "$CHROOT_DIR/boot/efi"

echo "Chrooting into the system"
sudo $DIVE_EXEC $CHROOT_DIR


echo "Cleaning up"

sudo umount -Rv "$CHROOT_DIR"

if is_luks "$ROOT_PART"; then
    echo "Closing the root partition"
    sudo cryptsetup close $ROOT_DEV
fi
