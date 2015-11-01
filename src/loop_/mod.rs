use mio;
use std::io;
use std::sync::mpsc::Sender;

mod handler;

use self::handler::Handler;
use {TokenFactory, Message, InputMessage, OutputMessage};

pub use self::handler::ClientStatistics;
pub type EventLoop<I, O> = mio::EventLoop<Handler<I, O>>;

pub struct Loop<I, O> where I: Message, O: Message {
    eloop:   EventLoop<I, O>,
    handler: Handler<I, O>,
}

impl<I, O> Loop<I, O> where I: Message, O: Message {
    pub fn new<F: TokenFactory + 'static> (factory: F, downstream: Sender<OutputMessage<O>>) -> Result<Loop<I, O>, io::Error> {
        let eloop = try!(EventLoop::new());
        let handler = Handler::new(factory, downstream);

        Ok(Loop {
            eloop: eloop,
            handler: handler,
        })
    }
}

impl<I, O> Loop<I, O> where I: Message, O: Message {
    pub fn channel (&self) -> mio::Sender<InputMessage<I>> {
        self.eloop.channel()
    }

    pub fn run (&mut self) -> Result<(), io::Error> {
        Ok(try!(self.eloop.run(&mut self.handler)))
    }
}
