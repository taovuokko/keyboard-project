use crate::Vec;

/// Configuration for MockRf behavior (immutable).
#[derive(Debug, Clone, Copy)]
pub struct MockRfConfig {
    pub drop_first: bool,
    pub reorder: bool,
    pub jitter_ms: u64,
}

/// Runtime state for MockRf (mutable).
struct MockRfRuntime {
    now_ms: u64,
    queue: Vec<(u64, Vec<u8>)>, // (deliver_at_ms, frame)
    delivered: usize,
    dropped: usize,
    first_frame_seen: bool,
}

/// Reusable mock RF channel that can drop, reorder, and inject jitter.
pub struct MockRf {
    config: MockRfConfig,
    runtime: MockRfRuntime,
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
            config: MockRfConfig {
                drop_first,
                reorder,
                jitter_ms,
            },
            runtime: MockRfRuntime {
                now_ms: 0,
                queue: Vec::new(),
                delivered: 0,
                dropped: 0,
                first_frame_seen: false,
            },
        }
    }

    pub fn advance(&mut self, delta_ms: u64) {
        self.runtime.now_ms = self.runtime.now_ms.saturating_add(delta_ms);
    }

    pub fn push(&mut self, frame: Vec<u8>) {
        // Drop first frame if configured and not yet seen
        if self.config.drop_first && !self.runtime.first_frame_seen {
            self.runtime.first_frame_seen = true;
            self.runtime.dropped += 1;
            return;
        }
        self.runtime.first_frame_seen = true;

        let deliver_at = self.runtime.now_ms + self.config.jitter_ms;
        self.runtime.queue.push((deliver_at, frame));

        if self.config.reorder && self.runtime.queue.len() >= 2 {
            let len = self.runtime.queue.len();
            self.runtime.queue.swap(len - 1, len - 2);
        }
    }

    pub fn pop(&mut self) -> Option<Vec<u8>> {
        if let Some(pos) = self
            .runtime
            .queue
            .iter()
            .position(|(deliver_at, _)| *deliver_at <= self.runtime.now_ms)
        {
            self.runtime.delivered += 1;
            Some(self.runtime.queue.remove(pos).1)
        } else {
            None
        }
    }

    pub fn stats(&self) -> MockRfStats {
        MockRfStats {
            delivered: self.runtime.delivered,
            dropped: self.runtime.dropped,
            last_time_ms: self.runtime.now_ms,
        }
    }
}
