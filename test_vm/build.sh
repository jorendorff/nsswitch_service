# build.sh - Build the nsswitch_service crate and run some unit tests.

set -eu

cd /nsswitch_service
cargo build
cargo build --examples
cargo test
