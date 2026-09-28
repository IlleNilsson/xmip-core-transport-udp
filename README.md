# xmip-core-transport-udp

UDP transport: one datagram is one Stream, no reply channel. A technology of
[xmip-core-transport](https://github.com/IlleNilsson/xmip-core-transport), which
owns the direction-neutral `Transport` trait this crate implements (ADR-0010).

Lifted out of the capability crate on 2026-09-07, where it had lived as
`src/udp` since 2026-08-27 waiting for this repository. The capability keeps
the trait, the error vocabulary and the shared wire helpers; nothing in it names
a protocol.

A Send Location sends from one socket per address family, bound on its first send and kept by the transport and its clones (`transport::sender::Sender`), so an IPv6 target is reached too; until 2026-09-27 every send bound a new IPv4 socket.

A Receive Location binds its socket on its first receive and keeps it (`transport::kept::Kept`), so a datagram that arrives between two receives waits in the socket's buffer for the next; until 2026-09-28 every receive bound a new socket and what came between two receives was lost.

`peer_of` reads the peer back out of an origin this technology wrote, `udp://<peer>`, for DDS, which names the peer in its own origin; until 2026-09-28 DDS cut it out itself.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
