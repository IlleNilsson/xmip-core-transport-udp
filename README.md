# xmip-core-transport-udp

UDP transport: one datagram is one Stream, no reply channel. A technology of
[xmip-core-transport](https://github.com/IlleNilsson/xmip-core-transport), which
owns the direction-neutral `Transport` trait this crate implements (ADR-0010).

Lifted out of the capability crate on 2026-09-07, where it had lived as
`src/udp` since 2026-08-27 waiting for this repository. The capability keeps
the trait, the error vocabulary and the shared wire helpers; nothing in it names
a protocol.

A Send Location sends from one socket per address family, bound on its first send and kept by the transport and its clones (`transport::sender::Sender`), so an IPv6 target is reached too; until 2026-09-27 every send bound a new IPv4 socket.

A Receive Location binds its socket as its transport is opened (`Configured::listening`; on its first receive until 2026-11-16) and keeps it (`transport::kept::Kept`), so a datagram that arrives between two receives waits in the socket's buffer for the next; until 2026-09-28 every receive bound a new socket and what came between two receives was lost.

`peer_of` reads the peer back out of an origin this technology wrote, `udp://<peer>`, for DDS, which names the peer in its own origin; until 2026-09-28 DDS cut it out itself.

## Acknowledgement

Acceptance is at-most-once here. A datagram has no reply: its sender is gone
once it is sent, so nobody is told how the receive cycle ended, and a crash
before the Stream is durable loses it. Each datagram arrives whole.

## The declaration

What a Location of this technology is configured with is two `const`s in its
`declaration` module, `src/declaration.rs`, beside the code, which
`Configured` reads (ADR-0064, amendment 2026-11-26):
`declaration::SETTINGS`, `timeout`, and `SCHEMES`, `udp`. The scheme an
origin opens with, `SCHEME`, is there too. Built with its `declaration`
feature alone (`--no-default-features --features declaration`), the crate is
those and nothing more: its code and every crate the code is built on are
behind the default feature. So the library the surfaces load carries it
without the code, through the gathering crate xmip-core-declaration (open
problem 48), and the language server and the desktop suggest and check a UDP
Location's settings though no node in their process loaded the technology.
`test/Declaration.Test.ps1` in the estate holds that the declaration builds
nothing the transport capability does not.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
