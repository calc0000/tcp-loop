use mio;
use std::io;
use std::sync::mpsc::Sender;

mod handler;

use self::handler::Handler;
use {TokenFactory, InputMessage, OutputMessage};

pub use self::handler::ClientStatistics;
pub type EventLoop = mio::EventLoop<Handler>;

pub struct Loop {
    eloop:   EventLoop,
    handler: Handler,
}

impl Loop {
    pub fn new<F: TokenFactory + 'static> (factory: F, downstream: Sender<OutputMessage>) -> Result<Loop, io::Error> {
        let eloop = try!(EventLoop::new());
        let handler = Handler::new(factory, downstream);

        Ok(Loop {
            eloop: eloop,
            handler: handler,
        })
    }
}

impl Loop {
    pub fn channel (&self) -> mio::Sender<InputMessage> {
        self.eloop.channel()
    }

    pub fn run (&mut self) -> Result<(), io::Error> {
        Ok(try!(self.eloop.run(&mut self.handler)))
    }
}
