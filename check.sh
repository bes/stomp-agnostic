set -e

cargo rustdoc -- -D rustdoc::broken_intra_doc_links
cargo clippy
cargo test
cargo fmt --check
