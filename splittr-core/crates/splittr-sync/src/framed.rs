//! A [`Transport`] over any ordered, reliable async byte stream — the bridge
//! between the pure protocol and a real network link (#20).
//!
//! Messages are length-prefixed postcard frames: a 4-byte big-endian length then
//! the bytes. This works over *any* `AsyncRead` + `AsyncWrite` pair, which is
//! precisely what an **iroh** bidirectional QUIC stream gives you — so the iroh
//! transport is a ~10-line wrapper around this, with no extra protocol code:
//!
//! ```ignore
//! // device_key (#16) is the iroh NodeId; invites (#7) carry NodeAddr + relay hint.
//! let endpoint = iroh::Endpoint::builder()
//!     .secret_key(device_key)
//!     .bind()
//!     .await?;
//! // Initiator dials the peer; responder accepts an incoming connection.
//! let conn = endpoint.connect(peer_addr, SPLITTR_ALPN).await?;
//! let (send, recv) = conn.open_bi().await?;          // accept_bi() on the responder
//! let mut transport = FramedTransport::new(recv, send);
//! splittr_sync::sync(&mut repo, &mut transport, Role::Initiator).await?;
//! ```
//!
//! Keeping the framing here means it is exercised over an in-memory byte duplex
//! in tests — the same code path a QUIC stream drives — so only the iroh
//! connection *setup* (untestable without a network) lives outside this crate.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::message::SyncMessage;
use crate::transport::Transport;

/// Upper bound on a single frame, so a hostile/garbled peer can't drive an
/// unbounded allocation. 64 MiB comfortably covers any op batch.
const MAX_FRAME_LEN: u32 = 64 * 1024 * 1024;

/// A [`Transport`] framing [`SyncMessage`]s over a split async byte stream.
pub struct FramedTransport<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> FramedTransport<R, W> {
    /// Wrap the read and write halves of a connected, ordered, reliable stream
    /// (e.g. the `(RecvStream, SendStream)` of an iroh bi-stream).
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }
}

/// A framed-transport failure: an IO error, an oversized frame, or undecodable
/// bytes (a malformed/incompatible peer).
#[derive(Debug)]
pub enum FramedError {
    Io(std::io::Error),
    FrameTooLarge,
    Decode,
}

impl std::fmt::Display for FramedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FramedError::Io(e) => write!(f, "transport io error: {e}"),
            FramedError::FrameTooLarge => f.write_str("sync frame exceeds the maximum size"),
            FramedError::Decode => f.write_str("malformed sync frame"),
        }
    }
}

impl std::error::Error for FramedError {}

impl From<std::io::Error> for FramedError {
    fn from(e: std::io::Error) -> Self {
        FramedError::Io(e)
    }
}

impl<R, W> Transport for FramedTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    type Error = FramedError;

    async fn send(&mut self, msg: SyncMessage) -> Result<(), FramedError> {
        let bytes = msg.to_bytes();
        let len = u32::try_from(bytes.len()).map_err(|_| FramedError::FrameTooLarge)?;
        if len > MAX_FRAME_LEN {
            return Err(FramedError::FrameTooLarge);
        }
        self.writer.write_all(&len.to_be_bytes()).await?;
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<SyncMessage, FramedError> {
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf);
        if len > MAX_FRAME_LEN {
            return Err(FramedError::FrameTooLarge);
        }
        let mut buf = vec![0u8; len as usize];
        self.reader.read_exact(&mut buf).await?;
        SyncMessage::from_bytes(&buf).ok_or(FramedError::Decode)
    }
}
