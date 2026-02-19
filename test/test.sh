#!/bin/bash

mkdir -p work/install_root
cd work

if [ ! -f install.img ]; then
    wget https://images.fyralabs.com/images/ultramarine/43/plasma-base-disk-$(arch).img.zst -O install.img.zst
    unzstd install.img.zst
fi

install_device=$(sudo losetup --nooverlap --partscan --show -f install.img)
sudo mount ${install_device}p3 install_root
sudo mount ${install_device}p2 install_root/boot
sudo mount ${install_device}p1 install_root/boot/efi

if [ ! -f test.img ]; then
    fallocate -l 20G test.img
fi

test_device=$(sudo losetup --nooverlap --partscan --show -f test.img)

TEST_DESTINATION_DISK=${test_device} TEST_REPART_DIRECTORY=../repart TEST_REPART_COPY_SOURCE=install_root envsubst < ../playbook.json.template > playbook.json

cargo build -p readymade-playbook --bin readymade-playbook --release

sudo ../../target/release/readymade-playbook playbook.json

isotovideo --exit-status-from-test-results UEFI=1 UEFI_PFLASH_CODE=/usr/share/edk2/ovmf/OVMF_CODE.fd UEFI_PFLASH_VARS=/usr/share/edk2/ovmf/OVMF_VARS.fd CASEDIR=../distri HDD_1=./test.img
