use mio::{TryRead, TryWrite};

use std::io;
use std::net;

use super::Stream;

pub enum OperationResult {
    Success(usize),
    WouldBlock,
}

pub struct Client {
    pub addr:     net::SocketAddr,
    stream:       Stream,

    write_buffer: Vec<u8>,
}

impl Client {
    pub fn new (addr: net::SocketAddr, stream: Stream) -> Client {
        Client {
            addr: addr,
            stream: stream,
            write_buffer: Vec::new(),
        }
    }

    pub fn as_ref (&self) -> &Stream {
        &self.stream
    }
}

// read functions
impl Client {
    pub fn try_read_all (&mut self) -> Result<Option<Vec<u8>>, io::Error> {
        let mut ret = Vec::new();

        let mut buf = &mut [0u8; 1024];

        while let Some(read) = try!(self.stream.read_slice(buf)) {
            if read == 0 {
                break;
            }

            ret.extend(buf[..read].iter().map(|x| *x));
        }

        Ok(Some(ret))
    }
}

// write functions
impl Client {
    pub fn queue_write (&mut self, data: &[u8]) -> Result<(), io::Error> {
        self.write_buffer.extend(data.iter().map(|x| *x));
        Ok(())
    }

    pub fn flush_write (&mut self) -> Result<OperationResult, io::Error> {
        match try!(self.stream.write_slice(&self.write_buffer[..])) {
            None => Ok(OperationResult::WouldBlock),
            Some(s) => {
                self.write_buffer = self.write_buffer[s..].to_vec();
                Ok(OperationResult::Success(s))
            },
        }
    }
}
