use Token;

use std::{io, net};

#[derive(Debug)]
pub enum Input {
    /// request that the loop listen on an address
    ///
    /// If the listen succeeds, an Output::ListenResponse will be sent to
    /// the downstream.  If it fails, an Output::Disconnect will be sent instead.
    ListenRequest {
        listener: Token,
        addr:     net::SocketAddr,
    },

    /// request that the loop establish a connection to an address
    ///
    /// If the connection succeeds, an Output::ConnectResponse will be sent to
    /// the downstream.  If it fails, an Output::Disconnect will be sent instead.
    ConnectRequest {
        token: Token,
        addr:  net::SocketAddr,
    },

    /// send some data to a client
    ///
    /// The loop will buffer data if necessary.
    Data {
        token: Token,
        data:  Vec<u8>,
    },

    /// request that a connection should be closed
    ///
    /// Can apply to either a listener or client.  The loop will send an Output::Close
    /// once the connection has been closed.
    Close {
        token: Token,
        dirty: bool,
    },

    /// request the loop to shutdown
    ///
    /// The loop will produce Output::Close messages for each client it disconnects as the
    /// loop shuts down.
    Shutdown,
}

#[derive(Debug)]
pub enum Output {
    ListenResponse {
        listener: Token,
    },

    ConnectRequest {
        listener: Token,
        client:   Token,
        addr:     net::SocketAddr,
    },

    ConnectResponse {
        token: Token,
    },

    Data {
        token: Token,
        data:  Vec<u8>,
    },

    Close {
        token:  Token,
    },
    DirtyClose {
        token:  Token,
        reason: Option<io::Error>,
    }
}
