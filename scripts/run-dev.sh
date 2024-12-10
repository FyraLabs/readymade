#!/bin/bash
# Run against the dev evnironment setup by setup-dev.sh

RUST_BACKTRACE=full READYMADE_LOG=trace READYMADE_CONFIG=templates/ultramarine-chromebook.toml REPART_COPY_SOURCE=$(pwd)/dev/install_root READYMADE_DRY_RUN=0 cargo run
