#![forbid(unsafe_code)]

//! Streams that arrive as datagrams. One datagram is one Stream.
//!
//! There is no reply channel and no delivery guarantee, which makes this the
//! clearest case of a transport that **cannot answer**: a Contract failure here
//! is audited and nothing more. HTTP is the same shape with a reply channel, and
//! the two behave differently at the gate for exactly that reason.

use std::net::UdpSocket;
use std::time::Duration;

use net::{Target, ceiling};
use transport::Arrived;
use transport::Configured;
use transport::Directions;
use transport::Transport;
use transport::bound::{Bound, Reading};
use transport::error::{Result, classify};
use transport::kept::Kept;
use transport::loopback::{FarEnd, LOOPBACK_TIMEOUT, Loopback};
use transport::sender::Sender;
use transport::socket;
use xcore::settings::{Applies, Kind, Presence, Read, Setting, Settings};

/// The largest a UDP payload can be over IPv4: 65535 less the 8-byte UDP header
/// and the 20-byte IP header (RFC 791, RFC 768).
pub const MAX_DATAGRAM: usize = 65_507;

#[derive(Clone)]
pub struct UdpTransport {
    bind: String,
    max_datagram: usize,
    receive_timeout: Option<Duration>,
    /// The socket every send leaves from, bound once.
    sender: Sender,
    /// The socket the first receive binds, and every receive reads.
    receiving: Kept<UdpSocket>,
}

impl UdpTransport {
    #[must_use]
    pub fn new(bind: impl Into<String>) -> Self {
        Self {
            bind: bind.into(),
            max_datagram: MAX_DATAGRAM,
            receive_timeout: None,
            sender: Sender::new(),
            receiving: Kept::new(),
        }
    }

    /// Give up waiting for a datagram that never comes. UDP has no delivery
    /// guarantee, so a receiver that does not time out waits forever when the
    /// datagram is dropped.
    #[must_use]
    pub const fn timing_out_after(mut self, timeout: Duration) -> Self {
        self.receive_timeout = Some(timeout);
        self
    }

    /// Bind and report the address actually assigned.
    ///
    /// Binding to port 0 lets the operating system choose. A datagram receiver
    /// must be bound before the sender fires, or the datagram is dropped
    /// silently — so a caller binds, learns the address, starts the sender, then
    /// calls [`receive_one`](Self::receive_one). This mirrors `TcpTransport`.
    ///
    /// # Errors
    ///
    /// Where the address is taken, malformed, or the read timeout cannot be set.
    pub fn bind(&self) -> Result<(UdpSocket, String)> {
        socket::bind_udp(&self.bind, self.receive_timeout)
    }

    /// Take one datagram from an already-bound socket.
    ///
    /// # Errors
    ///
    /// Where the datagram could not be received before the timeout.
    pub fn receive_one(&self, socket: &UdpSocket) -> Result<Arrived> {
        let mut buffer = vec![0u8; self.max_datagram];
        let (read, peer) = socket
            .recv_from(&mut buffer)
            .map_err(|e| classify("receiving a datagram", &e))?;

        buffer.truncate(read);

        Ok(Arrived::new(format!("{SCHEME}://{peer}"), buffer))
    }
}

/// The scheme an origin opens with: `udp://<peer>`.
const SCHEME: &str = "udp";

/// The peer an origin this transport wrote names — `10.0.0.5:7400` of
/// `udp://10.0.0.5:7400` — or the origin whole where it is not one: what
/// the protocols riding on UDP name the peer by in their own origins,
/// read here where the origin is written rather than by each of them.
#[must_use]
pub fn peer_of(origin: &str) -> &str {
    Target::under(&[SCHEME], origin).map_or(origin, |named| named.authority())
}

impl Transport for UdpTransport {
    fn name(&self) -> &'static str {
        "udp"
    }

    fn directions(&self) -> Directions {
        Directions::BOTH
    }

    /// One datagram, from the socket the first receive bound and kept: what
    /// arrived between two receives waits in its buffer.
    fn receive(&self) -> Result<Vec<Arrived>> {
        let socket = self.receiving.bound(|| self.bind())?;
        Ok(vec![self.receive_one(socket)?])
    }

    fn send(&self, target: &str, bytes: &[u8]) -> Result<()> {
        self.sender.send_to(bytes, target)
    }
}

impl Configured for UdpTransport {
    /// The address is where a Receive Location binds; a Send Location's
    /// target is each send's own. The one setting bounds a receive's wait.
    const SETTINGS: &'static Settings = &Settings {
        technology: env!("CARGO_PKG_NAME"),
        settings: &[Setting {
            name: "timeout",
            kind: Kind::Duration,
            presence: Presence::Optional,
            meaning: "How long a receive waits for a datagram; unbounded when left out.",
            applies: Applies::Receive,
        }],
    };

    fn configured(address: &str, settings: &Read) -> Result<Self> {
        let transport = Self::new(address);
        Ok(match settings.optional_duration("timeout") {
            Some(timeout) => transport.timing_out_after(timeout),
            None => transport,
        })
    }
}

impl UdpTransport {
    /// Both ends on this machine: an ephemeral local port, the loopback
    /// timeout on the receive — a datagram that never arrives is a timeout,
    /// which is what UDP is.
    #[must_use]
    pub fn loopback() -> Self {
        Self::new("127.0.0.1:0").timing_out_after(LOOPBACK_TIMEOUT)
    }
}

impl Reading for UdpTransport {
    /// A bound socket waiting for its one datagram. Bound before the sender
    /// fires, or the datagram is gone.
    fn take_one(self, socket: &UdpSocket) -> Result<Arrived> {
        self.receive_one(socket)
    }
}

impl Loopback for UdpTransport {
    fn ceiling(&self) -> Option<usize> {
        Some(self.max_datagram)
    }

    fn far_end(&self) -> Result<Box<dyn FarEnd>> {
        Ok(Box::new(Bound::new(self.clone(), self.bind()?)))
    }

    fn send_to(&self, address: &str, payload: &[u8]) -> Result<()> {
        ceiling::within(payload.len(), self.max_datagram, "one datagram carries")?;
        Self::new("127.0.0.1:0").send(address, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcore::settings::Given;

    #[test]
    fn udp_declares_its_settings_and_reads_through_them() {
        assert_eq!(UdpTransport::SETTINGS.problems(), Vec::<String>::new());
        let given = [("timeout".to_string(), Given::Text("250ms".to_string()))];
        let transport =
            UdpTransport::open("127.0.0.1:0", Applies::Receive, &given).expect("configured");
        assert_eq!(transport.bind, "127.0.0.1:0");
        assert_eq!(transport.receive_timeout, Some(Duration::from_millis(250)));
        let Err(refused) = UdpTransport::open("127.0.0.1:0", Applies::Send, &given) else {
            panic!("a Send Location reads no timeout");
        };
        assert!(refused.message.contains("timeout"), "{}", refused.message);
    }

    #[test]
    fn udp_round_trip_carries_bytes_and_peer() {
        // Bound here rather than inside the transport, because the receiver has
        // to be listening before the datagram is sent. UDP drops it silently
        // otherwise, and the test would hang rather than fail.
        let socket = UdpSocket::bind("127.0.0.1:0").expect("binding");
        let address = socket
            .local_addr()
            .expect("reading the address")
            .to_string();

        let sender = std::thread::spawn(move || {
            UdpTransport::new("127.0.0.1:0")
                .send(&address, b"hello over udp")
                .expect("sending");
        });

        let mut buffer = vec![0u8; MAX_DATAGRAM];
        let (read, peer) = socket.recv_from(&mut buffer).expect("receiving");
        sender.join().expect("the sending thread panicked");

        buffer.truncate(read);

        assert_eq!(buffer, b"hello over udp");
        assert!(peer.to_string().starts_with("127.0.0.1:"));
    }

    #[test]
    fn every_send_leaves_from_the_one_socket_the_transport_bound() {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("binding");
        let address = socket.local_addr().expect("address").to_string();
        let transport = UdpTransport::new("127.0.0.1:0");
        let mut peers = Vec::new();
        for n in 0..10u8 {
            transport.clone().send(&address, &[n]).expect("sent");
            let mut buffer = [0u8; 2];
            peers.push(socket.recv_from(&mut buffer).expect("received").1);
        }
        peers.dedup();
        assert_eq!((peers.len(), transport.sender.bound()), (1, 1));
    }

    #[test]
    fn bind_reports_its_address_so_a_sender_can_aim() {
        // The reason bind and receive_one are split: the receiver must be bound
        // and its address known before the sender fires a datagram at it.
        let receiver = UdpTransport::new("127.0.0.1:0").timing_out_after(Duration::from_secs(2));
        let (socket, address) = receiver.bind().expect("binding");

        let sender = std::thread::spawn(move || {
            UdpTransport::new("127.0.0.1:0")
                .send(&address, b"aimed over udp")
                .expect("sending");
        });

        let arrived = receiver.receive_one(&socket).expect("receiving");
        sender.join().expect("the sending thread panicked");

        assert_eq!(arrived.bytes, b"aimed over udp");
        assert!(arrived.origin_uri.starts_with("udp://127.0.0.1:"));
        assert!(peer_of(&arrived.origin_uri).starts_with("127.0.0.1:"));
    }

    #[test]
    fn every_receive_reads_the_socket_the_first_bound() {
        let receiver = UdpTransport::loopback();
        receiver.receiving.bound(|| receiver.bind()).expect("bound");
        let address = receiver.receiving.address().expect("address");
        transport::kept::held_across_receives(&receiver, address, 5, |at, payload| {
            UdpTransport::loopback().send(at, payload)
        });
        assert_eq!(receiver.receiving.address(), Some(address), "bound once");
    }

    #[test]
    fn a_datagram_socket_has_no_artefact_to_claim() {
        assert!(UdpTransport::new("127.0.0.1:0").claims().is_none());
    }
}
