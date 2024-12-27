#!/bin/bash -x

# Cleanup artifacts from setup-dev and run-dev

sudo umount dev/install_root -R
sudo rm -rf dev/test.img
sudo losetup -D
fallocate -l 8G dev/test.img