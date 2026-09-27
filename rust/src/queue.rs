use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

struct Inner<T> {
    items: VecDeque<T>,
    closed: bool,
}

pub struct BoundedQueue<T> {
    inner: Mutex<Inner<T>>,
    capacity: usize,
    not_full: Condvar,
    not_empty: Condvar,
}

impl<T> BoundedQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                items: VecDeque::with_capacity(capacity),
                closed: false,
            }),
            capacity,
            not_full: Condvar::new(),
            not_empty: Condvar::new(),
        }
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if guard.closed {
                return Err(value);
            }
            if guard.items.len() < self.capacity {
                guard.items.push_back(value);
                drop(guard);
                self.not_empty.notify_one();
                return Ok(());
            }
            guard = self.not_full.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn try_push(&self, value: T) -> Result<(), T> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.closed || guard.items.len() >= self.capacity {
            return Err(value);
        }
        guard.items.push_back(value);
        drop(guard);
        self.not_empty.notify_one();
        Ok(())
    }

    pub fn pop(&self) -> Option<T> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let item = guard.items.pop_front();
        if item.is_some() {
            drop(guard);
            self.not_full.notify_one();
        }
        item
    }

    pub fn wait_pop(&self) -> Option<T> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(item) = guard.items.pop_front() {
                drop(guard);
                self.not_full.notify_one();
                return Some(item);
            }
            if guard.closed {
                return None;
            }
            guard = self
                .not_empty
                .wait(guard)
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .items
            .len()
    }

    pub fn close(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.closed = true;
        drop(guard);
        self.not_full.notify_all();
        self.not_empty.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedQueue;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn push_pop_order_and_close() {
        let queue = BoundedQueue::new(2);
        queue.push(1).unwrap();
        queue.push(2).unwrap();
        assert_eq!(queue.try_push(3), Err(3));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pop(), Some(1));
        queue.close();
        assert!(queue.push(4).is_err());
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn wait_pop_wakes_on_close() {
        let queue = Arc::new(BoundedQueue::<u8>::new(1));
        let other = Arc::clone(&queue);
        let handle = std::thread::spawn(move || other.wait_pop());
        std::thread::sleep(Duration::from_millis(20));
        queue.close();
        assert_eq!(handle.join().unwrap(), None);
    }

    #[test]
    fn blocked_producer_wakes_on_drain() {
        let queue = Arc::new(BoundedQueue::new(1));
        queue.push(7).unwrap();
        let other = Arc::clone(&queue);
        let began = Instant::now();
        let handle = std::thread::spawn(move || other.push(9).is_ok());
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(queue.pop(), Some(7));
        assert!(handle.join().unwrap());
        assert!(began.elapsed() < Duration::from_secs(2));
    }
}
