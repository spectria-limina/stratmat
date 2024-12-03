use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

struct Inside<T> {
    val: Option<T>,
    wakers: Vec<Waker>,
}

impl<T> Default for Inside<T> {
    fn default() -> Self {
        Self {
            val: None,
            wakers: vec![],
        }
    }
}

#[derive(Clone)]
pub struct OnceTardis<T> {
    inside: Arc<Mutex<Inside<T>>>,
}

impl<T> Default for OnceTardis<T> {
    fn default() -> Self {
        Self {
            inside: Arc::new(Mutex::new(Inside::default())),
        }
    }
}

impl<T> OnceTardis<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, val: T) {
        let mut bigger = self.inside.lock().unwrap();
        if bigger.val.is_none() {
            bigger.val = Some(val);
            for w in bigger.wakers.drain(..) {
                w.wake()
            }
        }
    }
}

impl<T: Clone> Future for &OnceTardis<T> {
    type Output = T;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let mut bigger = self.inside.lock().unwrap();
        if let Some(ref val) = bigger.val {
            Poll::Ready(val.clone())
        } else {
            bigger.wakers.push(cx.waker().clone());
            Poll::Pending
        }
    }
}
