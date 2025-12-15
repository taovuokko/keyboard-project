use crate::Vec;

/// Reusable mock RF channel that can drop, reorder, and inject jitter.
pub struct MockRf {
    drop_first: bool,
    reorder: bool,
    jitter_ms: u64,
    now_ms: u64,
    queue: Vec<(u64, Vec<u8>)>, // (deliver_at_ms, frame)
    delivered: usize,
    dropped: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct MockRfStats {
    pub delivered: usize,
    pub dropped: usize,
    pub last_time_ms: u64,
}

impl MockRf {
    pub fn new(drop_first: bool, reorder: bool, jitter_ms: u64) -> Self {
        Self {
            drop_first,
            reorder,
            jitter_ms,
            now_ms: 0,
            queue: Vec::new(),
            delivered: 0,
            dropped: 0,
        }
    }

    pub fn advance(&mut self, delta_ms: u64) {
        self.now_ms = self.now_ms.saturating_add(delta_ms);
    }

    pub fn push(&mut self, frame: Vec<u8>) {
        if self.drop_first {
            self.drop_first = false;
            self.dropped += 1;
            return;
        }
        let deliver_at = self.now_ms + self.jitter_ms;
        self.queue.push((deliver_at, frame));
        if self.reorder && self.queue.len() >= 2 {
            let len = self.queue.len();
            self.queue.swap(len - 1, len - 2);
        }
    }

    pub fn pop(&mut self) -> Option<Vec<u8>> {
        if let Some(pos) = self
            .queue
            .iter()
            .position(|(deliver_at, _)| *deliver_at <= self.now_ms)
        {
            self.delivered += 1;
            Some(self.queue.remove(pos).1)
        } else {
            None
        }
    }

    pub fn stats(&self) -> MockRfStats {
        MockRfStats {
            delivered: self.delivered,
            dropped: self.dropped,
            last_time_ms: self.now_ms,
        }
    }
}
