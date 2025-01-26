#!/bin/bash
# Boots the system installed to dev/install.img, which is setup by the setup-dev.sh script and written to after a successful install using run-dev.sh
# 
# Note: Readymade (and dracut) usually expects to be run on the same system it is building for,
# so the image may not boot correctly inside a VM when installed using run-dev.sh.
# 
# You may need to run the rescue kernel and rebuild the initramfs inside the VM to get it to properly boot.
# 
# todo: Probably set `hostonly=no` in the config on dev builds of readymade to avoid this issue.

qemu-kvm \
    -machine type=q35,accel=kvm \
    -cpu host \
    -m 4G \
    -smp 4 \
    -drive file=dev/test.img,format=raw,if=virtio \
    -bios /usr/share/OVMF/OVMF_CODE.fd
