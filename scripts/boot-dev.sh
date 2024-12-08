#!/bin/bash
# Boots the system installed to dev/install.img, which is setup by the setup-dev.sh script and written to after a successful install using run-dev.sh

qemu-kvm \
    -machine type=q35,accel=kvm \
    -cpu host \
    -m 4G \
    -smp 4 \
    -drive file=dev/install.img,format=raw,if=virtio \
    -bios /usr/share/OVMF/OVMF_CODE.fd
