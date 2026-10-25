> libsodium-sys

# Build output ENV Variables
This is the possible build metadata for the crate.
* `DEP_SODIUM_INCLUDE` is the directory which contains the `sodium.h` header.
    It is only available if the header was installed and `SODIUM_LIB_DIR` was not set.
* `DEP_SODIUM_LIB` is the directory containing the compiled library.
    It is only available if `SODIUM_LIB_DIR` was not set.

See [`link build metadata`] for more information about build metadata.

[`link build metadata`]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key
