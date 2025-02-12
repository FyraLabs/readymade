#!/bin/bash -x

# Cleanup artifacts from setup-dev and run-dev

sudo umount dev/install_root -Rl
sudo rm -rf dev/test.img
sudo losetup -D
fallocate -l 20G dev/test.img
