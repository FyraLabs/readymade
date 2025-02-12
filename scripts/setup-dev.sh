#!/bin/bash
# Sets up a basic testing environment for development
# Usage: ./scripts/setup-dev.sh

# Step #0 - Setup directories

mkdir -p dev/install_root && cd dev

# Step #1 - Pull images for testing

if [ ! -f install.img ]; then
    wget https://images.fyralabs.com/images/ultramarine/40/base-base-disk-$(arch).img.zst -O install.img.zst
    unzstd install.img.zst
fi

# Step #2 - Setup disks

install_device=$(sudo losetup --nooverlap --partscan --show -f install.img)
sudo mount ${install_device}p3 install_root
sudo mount ${install_device}p2 install_root/boot
sudo mount ${install_device}p1 install_root/boot/efi

if [ ! -f test.img ]; then
    fallocate -l 20G test.img
fi

echo "Your test disk will be attached to the block device, install to it:"
sudo losetup --nooverlap --partscan --show -f test.img
