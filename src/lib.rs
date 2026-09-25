#![forbid(unsafe_code)]

//! Streams that arrive as datagrams. One datagram is one Stream.
//!
//! There is no reply channel and no delivery guarantee, which makes this the
//! clearest case of a transport that **cannot answer**: a Contract failure here
//! is audited and nothing more. HTTP is the same shape with a reply channel, and
//! the two behave differently at the gate for exactly that reason.

use std::net::UdpSocket;
use std::time::Duration;

use transport::Arrived;
use transport::Directions;
use transport::Transport;
use transport::bound::{Bound, Reading};
use transport::ceiling;
use transport::error::{Result, classify};
use transport::loopback::{FarEnd, LOOPBACK_TIMEOUT, Loopback};

/// The largest a UDP payload can be over IPv4: 65535 less the 8-byte UDP header
/// and the 20-byte IP header (RFC 791, RFC 768).
pub const MAX_DATAGRAM: usize = 65_507;

#[derive(Clone)]
pub struct UdpTransport {
    bind: String,
    max_datagram: usize,
    receive_timeout: Option<Duration>,
}

impl UdpTransport {
    #[must_use]
    pub fn new(bind: impl Into<String>) -> Self {
        Self {
            bind: bind.into(),
            max_datagram: MAX_DATAGRAM,
            receive_timeout: None,
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
        let socket = UdpSocket::bind(&self.bind).map_err(|e| classify("binding the socket", &e))?;

        if let Some(timeout) = self.receive_timeout {
            socket
                .set_read_timeout(Some(timeout))
                .map_err(|e| classify("setting the read timeout", &e))?;
        }

        let local = socket
            .local_addr()
            .map_err(|e| classify("reading the bound address", &e))?;

        Ok((socket, local.to_string()))
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

        Ok(Arrived::new(format!("udp://{peer}"), buffer))
    }
}

impl Transport for UdpTransport {
    fn name(&self) -> &'static str {
        "udp"
    }

    fn directions(&self) -> Directions {
        Directions::BOTH
    }

    fn receive(&self) -> Result<Vec<Arrived>> {
        let (socket, _) = self.bind()?;

        Ok(vec![self.receive_one(&socket)?])
    }

    fn send(&self, target: &str, bytes: &[u8]) -> Result<()> {
        let socket =
            UdpSocket::bind("0.0.0.0:0").map_err(|e| classify("binding the sending socket", &e))?;

        socket
            .send_to(bytes, target)
            .map_err(|e| classify("sending a datagram", &e))?;

        Ok(())
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
    }

    #[test]
    fn a_datagram_socket_has_no_artefact_to_claim() {
        assert!(UdpTransport::new("127.0.0.1:0").claims().is_none());
    }
}
