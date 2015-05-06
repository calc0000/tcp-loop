#[macro_use] extern crate log;
extern crate mio;

pub mod token_factory;
pub mod loop_;
pub mod message;

pub type Token = mio::Token;
pub use token_factory::Factory as TokenFactory;
pub use token_factory::SequentialFactory as SequentialTokenFactory;

pub use message::Output as OutputMessage;
pub use message::Input as InputMessage;

pub use loop_::Loop;

use std::sync::mpsc::{self, Sender, Receiver};
pub fn channel () -> (Sender<OutputMessage>, Receiver<OutputMessage>) {
    mpsc::channel()
}
