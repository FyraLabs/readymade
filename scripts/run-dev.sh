#!/bin/bash
# Run against the dev evnironment setup by setup-dev.sh

export READYMADE_LOG=trace
export READYMADE_CONFIG=templates/ultramarine.toml
export REPART_COPY_SOURCE=$(pwd)/dev/install_root
export RUST_BACKTRACE=full
export READYMADE_DRY_RUN=0
export READYMADE_REPART_DIR=$(pwd)/templates
cargo run
