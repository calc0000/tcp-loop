use mio::{TryRead, TryWrite};

use std::default::Default;
use std::io;
use std::net;

use super::Stream;

pub enum OperationResult {
    Success(usize),
    WouldBlock,
}

#[derive(Default, Debug, Clone)]
pub struct Statistics {
    pub bytes_read: u64,
    pub bytes_written_queued: u64,
    pub bytes_written: u64,
    pub blocked_writes: u64,
}

pub struct Client {
    pub addr:  net::SocketAddr,
    stream:    Stream,
    pub stats: Statistics,

    write_buffer: Vec<u8>,
}

impl Client {
    pub fn new (addr: net::SocketAddr, stream: Stream) -> Client {
        Client {
            addr: addr,
            stream: stream,
            stats: Default::default(),
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

            self.stats.bytes_read += read as u64;
            ret.extend(buf[..read].iter().map(|x| *x));
        }

        Ok(Some(ret))
    }
}

// write functions
impl Client {
    pub fn queue_write (&mut self, data: &[u8]) -> Result<(), io::Error> {
        self.write_buffer.extend(data.iter().map(|x| *x));
        self.stats.bytes_written_queued += data.len() as u64;
        Ok(())
    }

    pub fn flush_write (&mut self) -> Result<OperationResult, io::Error> {
        match try!(self.stream.write_slice(&self.write_buffer[..])) {
            None => {
                self.stats.blocked_writes += 1;
                Ok(OperationResult::WouldBlock)
            },
            Some(s) => {
                self.write_buffer = self.write_buffer[s..].to_vec();
                self.stats.bytes_written += s as u64;
                Ok(OperationResult::Success(s))
            },
        }
    }
}
