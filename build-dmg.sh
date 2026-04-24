#!/bin/bash
set -e

cargo build --release -p snapforge-ffi
mkdir -p qt/build
cd qt/build
cmake ..
make
make dmg