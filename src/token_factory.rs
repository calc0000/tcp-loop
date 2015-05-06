use mio;
use Token;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub trait Factory: Sync + Send {
    fn produce (&mut self) -> Token;
}

#[derive(Clone)]
pub struct SequentialFactory {
    counter: Arc<AtomicUsize>,
}

impl SequentialFactory {
    pub fn new () -> SequentialFactory {
        SequentialFactory {
            counter: Arc::new(AtomicUsize::new(1)),
        }
    }
}
impl Factory for SequentialFactory {
    fn produce (&mut self) -> Token {
        mio::Token(self.counter.fetch_add(1, Ordering::Relaxed))
    }
}
