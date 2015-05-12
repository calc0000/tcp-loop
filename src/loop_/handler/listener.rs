use std::ops::{Deref, DerefMut};
use mio;
use mio::tcp::TcpListener;

pub struct Listener {
    listener: mio::NonBlock<TcpListener>,
}

impl Listener {
    pub fn new (listener: mio::NonBlock<TcpListener>) -> Listener {
        Listener {
            listener: listener,
        }
    }
}

impl Deref for Listener {
    type Target = mio::NonBlock<TcpListener>;

    fn deref (&self) -> &mio::NonBlock<TcpListener> {
        &self.listener
    }
}
impl DerefMut for Listener {
    fn deref_mut (&mut self) -> &mut mio::NonBlock<TcpListener> {
        &mut self.listener
    }
}
