# Release to crates-io
cd rmk-macro
cargo release --registry crates-io patch --execute
cd rmk
cargo release --registry crates-io patch --execute