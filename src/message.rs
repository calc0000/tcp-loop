use ClientStatistics;
use Token;

use std::{io, net};

#[derive(Debug)]
pub enum Input {
    /// request that the loop listen on an address
    ///
    /// If the listen succeeds, an Output::ListenResponse will be sent to
    /// the downstream.  If it fails, an Output::Close will be sent instead.
    ListenRequest {
        /// the token to associate with this listener
        listener: Token,

        /// the address to listen on
        addr:     net::SocketAddr,
    },

    /// request that the loop establish a connection to an address
    ///
    /// If the connection succeeds, an Output::ConnectResponse will be sent to
    /// the downstream.  If it fails, an Output::Disconnect will be sent instead.
    ConnectRequest {
        /// the token to associate with this connection
        token: Token,

        /// the address to connect to
        addr:  net::SocketAddr,
    },

    /// send some data to a client
    ///
    /// The loop will buffer data to be written, if necessary.
    Data {
        /// the token associated with the connection to send data over
        token: Token,

        /// the data to send
        data:  Vec<u8>,
    },

    /// request statistics for a connection
    ///
    /// If the token is associated with a present and valid connection, an
    /// Output::StatisticsResponse will be sent to the downstream.
    StatisticsRequest {
        /// the token associated with the connection
        token: Token,
    },

    /// request that a connection should be closed
    ///
    /// Can apply to either a listener or client.  The loop will send an Output::Close
    /// once the connection has been closed.
    Close {
        /// the token associated with the connection or listener to close
        token: Token,

        /// whether this is a dirty disconnect or not
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
    /// indicate that a listener has been established
    ///
    /// This message is sent in response to an Input::ListenRequest that succeeds.
    ListenResponse {
        /// the token associated with the listener, specified by the Input::ListenRequest that
        /// created this listener
        listener: Token,
    },

    /// notify the downstream that a client connection has been accepted
    ///
    /// An Output::ConnectRequest message is generated in response to a client connection
    /// successfully being accepted by a listener.  By the time this message is produced, this
    /// connection has already been recorded in the loop's internal record, and has been registered
    /// in the event loop.
    ConnectRequest {
        /// the token associated with the listener that accepted the connection
        listener: Token,

        /// the token associated with the connection
        client:   Token,

        /// the address of the peer
        addr:     net::SocketAddr,
    },

    /// indicate that an outgoing connection has succeeded
    ///
    /// This message is sent in response to an Input::ConnectRequest that succeeds.  By the time
    /// this message is produced, the connection has been recorded in the loop's internal record,
    /// and has been registered in the event loop.
    ConnectResponse {
        /// the token associated with the connection, as specified in the Input::ConnectRequest
        token: Token,
    },

    /// notify the downstream that data has been read from a connection
    ///
    /// When a connection has been marked readable by the event loop and some data has been read,
    /// this message is produced and sent to the downstream.
    Data {
        /// the token associated with the connection
        token: Token,

        /// the data read from the connection
        data:  Vec<u8>,
    },

    /// send statistics for a client to the downstream
    StatisticsResponse {
        /// the token associated with the connection
        token: Token,

        /// the statistics
        stats: ClientStatistics,
    },

    /// notify the downstream that a connection has ended cleanly or a listener has stopped
    /// listening
    ///
    /// When a connection is terminated in a "clean" way (from the tcp perspective), this message
    /// is generated and sent to the downstream.  This disconnection can occur as a result of a
    /// number of different circumstances.
    ///
    /// This message is also generated when a listener has stopped accepting connections, as
    /// requested by the downstream.
    Close {
        /// the token associated with the connection or listener that has closed
        token:  Token,
    },

    /// notify the downstream that a connection ended uncleanly
    ///
    /// This message is generated when a connection ends as a result of some sort of error.  In
    /// most cases the error is included in the message.
    DirtyClose {
        /// the token associated with the connection that ended
        token:  Token,

        /// if present, the error associated with the end
        reason: Option<io::Error>,
    }
}
