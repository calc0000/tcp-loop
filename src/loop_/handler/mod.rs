use std::collections::HashMap;
use std::{convert, io, net};
use std::sync::mpsc::Sender;
use mio;
use mio::tcp::TcpStream;

use self::client::Client;
use self::listener::Listener;

use loop_::EventLoop;
use {InputMessage, OutputMessage};
use {Token, TokenFactory};

mod client;
mod listener;

pub use self::client::Statistics as ClientStatistics;
pub type Stream   = mio::NonBlock<TcpStream>;

#[derive(Debug)]
enum Action {
    None,
    TryFlush,
}

#[derive(Debug)]
enum Error {
    AcceptFailed,

    // these errors do not constitute shutting down of the loop, but do cause a client dirty
    // disconnect
    Io(io::Error),
    ClientError,

    // these errors constitute a clean client disconnect
    ClientDisconnect,

    // these errors do cause the loop to shutdown
    DownstreamDisconnect,
}

impl convert::From<io::Error> for Error {
    fn from (x: io::Error) -> Error {
        Error::Io(x)
    }
}

fn new_client (clients: &mut HashMap<Token, (bool, Client)>, eloop: &mut EventLoop, token: Token, addr: net::SocketAddr, stream: Stream) -> Result<(), Error> {
    let client = Client::new(addr.clone(), stream);

    info!("new client at {:?}", addr);

    // register with the event loop
    match eloop.register_opt(
        client.as_ref(),
        token,
        mio::Interest::readable() | mio::Interest::hup() | mio::Interest::error(),
        mio::PollOpt::level()
        ) {
            Err(e) => {
                error!("failed to register client at {:?}: {:?}", addr, e);
                return Err(Error::AcceptFailed);
            },
            _ => {},
        }

    // stash it in the HashMap
    clients.insert(token, (false, client));

    Ok(())
}

pub struct Handler {
    pending_clients: HashMap<Token, (net::SocketAddr, Stream)>,
    clients:         HashMap<Token, (bool, Client)>,
    listeners:       HashMap<Token, Listener>,
    downstream:      Sender<OutputMessage>,
    factory:         Box<TokenFactory + 'static>,
}

impl Handler {
    pub fn new<F: TokenFactory + 'static> (
        factory:    F,
        downstream: Sender<OutputMessage>,
        ) -> Handler {
            Handler {
                pending_clients: HashMap::new(),
                clients:         HashMap::new(),
                listeners:       HashMap::new(),
                downstream:      downstream,
                factory:         Box::new(factory),
            }
        }

    fn try_flush (&mut self, eloop: &mut EventLoop, token: Token) -> Result<Action, Error> {
        if let Some(&mut (ref mut waiting_for_write, ref mut client)) = self.clients.get_mut(&token) {
            // try to flush the client
            match client.flush_write() {
                // the write failed
                Err(e) => {
                    error!("error flushing write for client at {:?}: {:?}", client.addr, e);
                    return Err(Error::ClientError);
                },

                // the write would've blocked
                Ok(client::OperationResult::WouldBlock) => {

                    // were we waiting for it to be writeable?
                    if ! *waiting_for_write {
                        // reregister it with the writeable interest
                        match eloop.reregister(
                            client.as_ref(),
                            token,
                            mio::Interest::readable() | mio::Interest::writable() | mio::Interest::hup() | mio::Interest::error(),
                            mio::PollOpt::level()
                            ) {
                                Err(e) => {
                                    error!("failed to reregister client at {:?} for writeable: {:?}", client.addr, e);
                                    return Err(Error::Io(e));
                                },
                                _ => {},
                            }
                        *waiting_for_write = true;
                    }

                    Ok(Action::None)
                },

                // the write didn't block, so make sure that it's not waiting for a write
                Ok(client::OperationResult::Success(size)) => {
                    trace!("wrote {:?} bytes for client at {:?}", size, client.addr);

                    if *waiting_for_write {
                        // reregister it without the writeable interest
                        match eloop.reregister(
                            client.as_ref(),
                            token,
                            mio::Interest::readable() | mio::Interest::hup() | mio::Interest::error(),
                            mio::PollOpt::level()
                            ) {
                                Err(e) => {
                                    error!("failed to reregister client at {:?} for not writeable: {:?}", client.addr, e);
                                    return Err(Error::Io(e));
                                },
                                _ => {},
                            }
                        *waiting_for_write = false;
                    }

                    Ok(Action::None)
                },
            }
        } else {
            warn!("received flush request for stale token {:?}", token);
            Ok(Action::None)
        }
    }

    fn proc_listen_request (&mut self, eloop: &mut EventLoop, token: Token, addr: net::SocketAddr) -> Result<Action, Error> {
        let listener = try!(mio::tcp::listen(&addr));

        // register it in the loop
        match eloop.register_opt(
            &listener,
            token,
            mio::Interest::readable() | mio::Interest::hup() | mio::Interest::error(),
            mio::PollOpt::level()
            ) {
                Err(e) => {
                    error!("failed to register listener at {:?} for readable: {:?}", addr, e);
                    return Err(Error::Io(e));
                },
                _ => {},
            }

        debug!("listening on {:?}: {:?}", addr, token);

        // stuff it in the hash map
        self.listeners.insert(token, Listener::new(listener));
        
        // send response
        match self.downstream.send(OutputMessage::ListenResponse { listener: token }) {
            Err(_) => return Err(Error::DownstreamDisconnect),
            Ok(_)  => {},
        }

        Ok(Action::None)
    }

    fn proc_connect_request (&mut self, eloop: &mut EventLoop, token: Token, addr: net::SocketAddr) -> Result<Action, Error> {
        let (stream, waiting) = try!(mio::tcp::connect(&addr));

        if waiting {
            // register the stream for output
            match eloop.register_opt(
                &stream,
                token,
                mio::Interest::writable() | mio::Interest::hup() | mio::Interest::error(),
                mio::PollOpt::level()
                ) {
                    Err(e) => {
                        error!("failed to register client at {:?} for writeable: {:?}", &addr, e);
                        return Err(Error::Io(e));
                    },
                    _ => {},
                }

            // stuff it in the hash map
            self.pending_clients.insert(token, (addr, stream));

            Ok(Action::None)
        } else {
            // stick the new client in the hash map
            try!(new_client(&mut self.clients, eloop, token, addr, stream));

            Ok(Action::None)
        }
    }

    fn proc_data (&mut self, token: Token, data: Vec<u8>) -> Result<Action, Error> {
        if let Some(&mut (_, ref mut client)) = self.clients.get_mut(&token) {
            match client.queue_write(&data) {
                Err(e) => {
                    error!("error queuing data: {:?}", e);

                    Err(Error::ClientError)
                },
                Ok(_) => {
                    trace!("queued data for {:?}", client.addr);
                    Ok(Action::TryFlush)
                },
            }
        } else {
            warn!("received data request for stale token {:?}", token);
            Ok(Action::None)
        }
    }

    fn proc_stats_request (&mut self, token: Token) -> Result<Action, Error> {
        if let Some(&(_, ref client)) = self.clients.get(&token) {
            let stats = client.stats.clone();
            match self.downstream.send(OutputMessage::StatisticsResponse {
                token: token,
                stats: stats,
            }) {
                Err(_) => Err(Error::DownstreamDisconnect),
                Ok(_) => Ok(Action::None),
            }
        } else {
            warn!("received stats request for stale token {:?}", token);
            Ok(Action::None)
        }
    }

    fn proc_close (&mut self, eloop: &mut EventLoop, token: Token, dirty: bool, reason: Option<io::Error>) -> Result<Action, Error> {
        if self.clients.contains_key(&token) {
            if let Some(&mut (_, ref mut client)) = self.clients.get_mut(&token) {
                match eloop.deregister(client.as_ref()) {
                    Err(e) => return Err(Error::Io(e)),
                    _ => {},
                }
            }

            self.clients.remove(&token);
            // the client should be dropped here, causing the TCP close procedure

            // and finally, notify the downstream
            match if dirty {
                self.downstream.send(OutputMessage::DirtyClose { token: token, reason: reason })
            } else {
                self.downstream.send(OutputMessage::Close { token: token })
            } {
                Err(_) => return Err(Error::DownstreamDisconnect),
                Ok(_) => {},
            }
        }

        Ok(Action::None)
    }

    fn deregister_clients (&mut self, eloop: &mut EventLoop) -> Vec<Token> {
        let mut disconnected_clients = Vec::new();

        for (&token, &mut (_, ref mut client)) in self.clients.iter_mut() {
            disconnected_clients.push(token);
            match eloop.deregister(client.as_ref()) {
                Err(_) => {},
                Ok(_) => {},
            }
        }

        disconnected_clients
    }
    fn proc_shutdown (&mut self, eloop: &mut EventLoop) {
        let disconnected_clients = self.deregister_clients(eloop);
        self.clients.clear();   // the drop should trigger the TCP close sequence

        for token in disconnected_clients {
            match self.downstream.send(OutputMessage::Close { token: token }) {
                Err(_) => {},
                Ok(_) => {},
            }
        }

        eloop.shutdown();
    }

    fn accept (&mut self, eloop: &mut EventLoop, listener_token: Token) -> Result<(), Error> {
        if let Some(listener) = self.listeners.get_mut(&listener_token) {
            match listener.accept() {
                Err(e) => {
                    error!("failed to accept incoming connection: {:?}", e);
                    Err(Error::AcceptFailed)
                },
                Ok(None) => Ok(()),
                Ok(Some(stream)) => {
                    let addr = match stream.peer_addr() {
                        Err(e) => {
                            error!("failed to get peer addr: {:?}", e);
                            return Err(Error::AcceptFailed);
                        },
                        Ok(x) => x,
                    };

                    let token = self.factory.produce();

                    // stuff it in the hash map
                    try!(new_client(&mut self.clients, eloop, token, addr.clone(), stream));

                    match self.downstream.send(OutputMessage::ConnectRequest {
                        listener: listener_token,
                        client:   token,
                        addr:     addr,
                    }) {
                        Err(_) => return Err(Error::DownstreamDisconnect),
                        Ok(_)  => Ok(()),
                    }
                },
            }
        } else {
            warn!("attempted to accept from stale listener {:?}", listener_token);

            Ok(())
        }
    }

    fn readable (&mut self, eloop: &mut EventLoop, token: Token, hint: mio::ReadHint) -> Result<Action, Error> {
        if self.listeners.contains_key(&token) {
            match self.accept(eloop, token) {
                Err(e) => Err(e),
                Ok(_) => Ok(Action::None),
            }
        } else {
            if let Some(&mut (_, ref mut client)) = self.clients.get_mut(&token) {
                if hint.contains(mio::ReadHint::error()) {
                    // client read error
                    info!("error from client {:?}", client.addr);
                    return Err(Error::ClientError);
                }

                debug!("reading from {:?} at {:?}", token, client.addr);

                // try to read some data
                match client.try_read_all() {
                    Err(e) => {
                        info!("error reading data from client at {:?}: {:?}", client.addr, e);
                        return Err(Error::ClientError);
                    },

                    // would block
                    Ok(None) => {},

                    // we got data! if there's no bytes we don't send a message to the downstream,
                    // since we'll separately send a client disconnect (a zero length read is
                    // typically indicative of a hangup, which we check for later)
                    Ok(Some(data)) => if data.len() > 0 {
                        // kick the packets over to the downstream
                        match self.downstream.send(OutputMessage::Data {
                            token: token,
                            data:  data,
                        }) {
                            Err(_) => {
                                error!("downstream disconnected");
                                return Err(Error::DownstreamDisconnect);
                            },
                            _ => {},
                        }
                    },
                }

                if hint.contains(mio::ReadHint::hup()) {
                    // client hung up
                    info!("client at {:?} disconnected", client.addr);
                    return Err(Error::ClientDisconnect);
                }

                Ok(Action::None)
            } else {
                warn!("received readable event for stale token {:?}", token);

                Ok(Action::None)
            }
        }
    }

    fn writable (&mut self, eloop: &mut EventLoop, token: Token) -> Result<Action, Error> {
        if self.clients.contains_key(&token) {
            if let Some(&mut (ref mut waiting_for_write, _)) = self.clients.get_mut(&token) {
                if !*waiting_for_write {
                    error!("received writable event, but not waiting for write on token {:?}!", token);
                    return Ok(Action::None);
                }

                // now we fall through to the flush
            }

            self.try_flush(eloop, token)
        } else {
            if let Some((addr, stream)) = self.pending_clients.remove(&token) {
                // assume the connection succeeded
                try!(new_client(&mut self.clients, eloop, token, addr, stream));

                return Ok(Action::None);
            } else {
                warn!("received writable event for stale token {:?}", token);

                return Ok(Action::None);
            }
        }
    }

    fn handle_result (&mut self, eloop: &mut EventLoop, token: Token, res: Result<Action, Error>) {
        match res {
            Err(Error::AcceptFailed) => {}, // do nothing here for now

            // client dirty disconnect
            Err(Error::Io(_)) | Err(Error::ClientError) => {
                info!("dirty disconnect client {:?}", token);
                mio::Handler::notify(self, eloop, InputMessage::Close { token: token, dirty: true });
            },

            // client clean disconnect
            Err(Error::ClientDisconnect) => {
                info!("clean disconnect client {:?}", token);
                mio::Handler::notify(self, eloop, InputMessage::Close { token: token, dirty: false });
            },

            // these errors cause a loop shutdown
            Err(Error::DownstreamDisconnect) => eloop.shutdown(),

            Ok(Action::None) => {}, // nothing to do here

            Ok(Action::TryFlush) => {
                let result = self.try_flush(eloop, token);  // this will never return a TryFlush
                self.handle_result(eloop, token, result);
            },
        }
    }
}

impl mio::Handler for Handler {
    type Timeout = ();
    type Message = InputMessage;

    fn readable (&mut self, eloop: &mut EventLoop, token: Token, hint: mio::ReadHint) {
        let result = Handler::readable(self, eloop, token, hint);
        self.handle_result(eloop, token, result);
    }

    fn writable (&mut self, eloop: &mut EventLoop, token: Token) {
        let result = Handler::writable(self, eloop, token);
        self.handle_result(eloop, token, result);
    }

    fn notify (&mut self, eloop: &mut EventLoop, message: InputMessage) {
        let (token, result) = match message {
            InputMessage::ListenRequest { 
                listener: token,
                addr,
            } => (token, self.proc_listen_request(eloop, token, addr)),

            InputMessage::ConnectRequest {
                token,
                addr,
            } => (token, self.proc_connect_request(eloop, token, addr)),

            InputMessage::Data {
                token,
                data,
            } => (token, self.proc_data(token, data)),

            InputMessage::StatisticsRequest {
                token,
            } => (token, self.proc_stats_request(token)),

            InputMessage::Close {
                token,
                dirty,
            } => (token, self.proc_close(eloop, token, dirty, None)),

            InputMessage::Shutdown => {
                self.proc_shutdown(eloop);
                return;
            },
        };

        self.handle_result(eloop, token, result);
    }
}
