# parity-tokio-ipc

[![Build Status](https://travis-ci.org/NikVolf/parity-tokio-ipc.svg?branch=master)](https://travis-ci.org/NikVolf/parity-tokio-ipc)

[Documentation](https://nikvolf.github.io/parity-tokio-ipc)

This crate abstracts interprocess transport for UNIX/Windows. On UNIX it utilizes unix sockets (`tokio_uds` crate) and named pipe on windows (experimental `tokio-named-pipes` crate).

Endpoint is transport-agnostic interface for incoming connections:
```rust
  let endpoint = Endpoint::new(endpoint_addr, handle).unwrap();
  endpoint.incoming().for_each(|_| println!("Connection received!"));
```

And IpcStream is transport-agnostic io:
```rust
  let endpoint = Endpoint::new(endpoint_addr, handle).unwrap();
  endpoint.incoming().for_each(|(ipc_stream: IpcStream, _)| io::write_all(ipc_stream, b"Hello!"));
```


# License

`parity-tokio-ipc` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
