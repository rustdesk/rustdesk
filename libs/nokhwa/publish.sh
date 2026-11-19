cd nokhwa-core || exit
cargo publish
cd ../nokhwa-bindings-linux || exit
cargo publish
cd ../nokhwa-bindings-macos || exit
cargo publish
cd ../nokhwa-bindings-windows || exit
cargo publish
cd .. || exit
cargo publish
