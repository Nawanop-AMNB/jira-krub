use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

/// Runs blocking jobs on background threads and delivers results as messages.
pub struct Worker<M: Send + 'static> {
    tx: Sender<M>,
    rx: Receiver<M>,
}

impl<M: Send + 'static> Default for Worker<M> {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx }
    }
}

impl<M: Send + 'static> Worker<M> {
    /// Job returns one message when done.
    pub fn spawn<F>(&self, job: F)
    where
        F: FnOnce() -> M + Send + 'static,
    {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(job());
        });
    }

    /// Job may emit many messages (progress) through the given sender.
    pub fn spawn_streaming<F>(&self, job: F)
    where
        F: FnOnce(Sender<M>) + Send + 'static,
    {
        let tx = self.tx.clone();
        thread::spawn(move || job(tx));
    }

    pub fn drain(&self) -> Vec<M> {
        let mut out = Vec::new();
        while let Ok(m) = self.rx.try_recv() {
            out.push(m);
        }
        out
    }
}
