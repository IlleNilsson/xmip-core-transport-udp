//! This technology's declaration: the transport guide's "The declaration",
//! `module/core/capability/transport/doc/adding-a-transport.md`.

use xcore::settings::{Applies, Settings};

/// Every setting a Location takes beyond its address, which is where a
/// Receive Location binds; a Send Location's target is each send's own.
/// The one setting bounds a receive's wait.
pub const SETTINGS: &Settings = &Settings {
    technology: env!("CARGO_PKG_NAME"),
    settings: &[transport::configured::timeout(
        Applies::Receive,
        None,
        "How long a receive waits for a datagram; unbounded when left out.",
    )],
};

/// The scheme an origin opens with, `udp://<peer>`, and a Location's
/// address is entered with to choose this technology.
pub const SCHEME: &str = "udp";

/// This technology's schemes, as [`transport::Configured::SCHEMES`] defines
/// them.
pub const SCHEMES: &[&str] = &[SCHEME];
