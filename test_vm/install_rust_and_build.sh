# install_rust_and_build.sh - Script to install Rust and build the
# nsswitch_service crate, build its example library, and run unit tests.

set -eu

curl https://sh.rustup.rs -sSf | sh -- /dev/stdin -y

cd /nsswitch_service
cargo build
cargo build --examples
cargo test
