use parking_lot::{Condvar, Mutex};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct WorkerPool {
    threads: Vec<JoinHandle<()>>,
    queue: Arc<(Mutex<VecDeque<Job>>, Condvar)>,
    shutdown: Arc<AtomicBool>,
}

impl WorkerPool {
    pub fn new(num_threads: usize) -> Self {
        let queue = Arc::new((Mutex::new(VecDeque::<Job>::new()), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::with_capacity(num_threads);

        for i in 0..num_threads {
            let q = Arc::clone(&queue);
            let s = Arc::clone(&shutdown);
            let handle = thread::Builder::new()
                .name(format!("titankv-worker-{}", i))
                .spawn(move || {
                    while !s.load(Ordering::Relaxed) {
                        let job = {
                            let (ref lock, ref cvar) = *q;
                            let mut queue_guard = lock.lock();
                            while queue_guard.is_empty() && !s.load(Ordering::Relaxed) {
                                cvar.wait(&mut queue_guard);
                            }
                            queue_guard.pop_front()
                        };

                        if let Some(task) = job {
                            task();
                        }
                    }
                })
                .expect("Failed to spawn worker thread");
            threads.push(handle);
        }

        Self {
            threads,
            queue,
            shutdown,
        }
    }

    pub fn spawn<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let (ref lock, ref cvar) = *self.queue;
        lock.lock().push_back(Box::new(task));
        cvar.notify_one();
    }

    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.queue.1.notify_all();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}
