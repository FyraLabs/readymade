#!/bin/bash
# Run against the dev evnironment setup by setup-dev.sh

export READYMADE_LOG=trace,backhand=debug
export READYMADE_CONFIG=templates/ultramarine.toml
export REPART_COPY_SOURCE=$(pwd)/dev/install_root
# : ${REPART_COPY_SOURCE:=$(pwd)/dev/install_root}
# export REPART_COPY_SOURCE=$REPART_COPY_SOURCE
# export REPART_COPY_SOURCE=/home/mado/ドキュメント/images/beta/ultramarine-plasma-rdm2-41-live-x86_64/LiveOS/squashfs.img
export RUST_BACKTRACE=full
export READYMADE_DRY_RUN=0
export READYMADE_REPART_DIR=$(pwd)/templates
cargo run
