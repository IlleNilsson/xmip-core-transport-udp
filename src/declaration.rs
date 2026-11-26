//! What a Location of this technology is configured with: its declaration,
//! which `Configured` reads, built with the crate's `declaration` feature
//! alone and the code behind the default (ADR-0064, amendments 2026-10-22
//! and 2026-11-26), for the library the surfaces load to carry (open
//! problem 48).

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

/// The schemes a Location's address is entered with to choose this technology.
pub const SCHEMES: &[&str] = &[SCHEME];
