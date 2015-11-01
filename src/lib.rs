#[macro_use] extern crate log;
extern crate mio;

pub mod token_factory;
pub mod loop_;
pub mod message;

// re-export these types for consumer convenience
pub use std::sync::mpsc::{Sender, Receiver};

pub type Token = mio::Token;
pub use token_factory::Factory as TokenFactory;
pub use token_factory::SequentialFactory as SequentialTokenFactory;

pub use message::Message as Message;
pub use message::Output as OutputMessage;
pub use message::Input as InputMessage;

pub use loop_::Loop;
pub use loop_::ClientStatistics;

pub fn channel<O> () -> (Sender<OutputMessage<O>>, Receiver<OutputMessage<O>>) where O: Message {
    use std::sync::mpsc;
    mpsc::channel()
}
