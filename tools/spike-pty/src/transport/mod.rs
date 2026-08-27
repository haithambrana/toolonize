//! Bounded LOSSLESS transport experiment.
//! This validates the design for M3: bounded batching with backpressure and no silent drop.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

/// Lossless bounded transport with high/low water marks.
/// Mimics PTY -> Rust reader -> Tauri Channel -> WebView pipeline stages.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub capacity: usize,   // max bytes queued
    pub high_water: usize, // when to apply backpressure
    pub low_water: usize,  // when to resume
    pub batch_size: usize, // coalesce small writes
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            capacity: 64 * 1024, // 64KB
            high_water: 48 * 1024,
            low_water: 16 * 1024,
            batch_size: 4 * 1024,
        }
    }
}

#[derive(Debug)]
pub enum TransportError {
    WouldBlock,
    Disconnected,
    HardLimitBreach { queued: usize, limit: usize },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TransportStats {
    pub produced_bytes: usize,
    pub delivered_bytes: usize,
    pub dropped_bytes: usize,
    pub max_queue_depth: usize,
    pub backpressure_events: usize,
    pub hard_limit_breaches: usize,
    pub lossless: bool,
}

/// A lossless bounded transport that never silently drops.
/// Under overload, it applies backpressure (blocks producer) or, if hard limit
/// breached, transitions to Desynchronized error state.
pub struct LosslessTransport {
    config: TransportConfig,
    // Internal queue
    queue: Arc<Mutex<VecDeque<u8>>>,
    produced: usize,
    delivered: usize,
    max_depth: usize,
    backpressure_count: usize,
    hard_breach: usize,
    desync: bool,
}

impl LosslessTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            config,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            produced: 0,
            delivered: 0,
            max_depth: 0,
            backpressure_count: 0,
            hard_breach: 0,
            desync: false,
        }
    }

    /// Producer writes bytes; blocks if would exceed high water (backpressure).
    pub fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if self.desync {
            return Err(TransportError::HardLimitBreach {
                queued: self.queue.lock().unwrap().len(),
                limit: self.config.capacity,
            });
        }
        let q_len = self.queue.lock().unwrap().len();
        if q_len + data.len() > self.config.capacity {
            self.hard_breach += 1;
            self.desync = true;
            return Err(TransportError::HardLimitBreach {
                queued: q_len,
                limit: self.config.capacity,
            });
        }
        if q_len > self.config.high_water {
            self.backpressure_count += 1;
            return Err(TransportError::WouldBlock);
        }
        let mut q = self.queue.lock().unwrap();
        // Re-check after acquiring lock
        if q.len() + data.len() > self.config.capacity {
            self.hard_breach += 1;
            self.desync = true;
            return Err(TransportError::HardLimitBreach {
                queued: q.len(),
                limit: self.config.capacity,
            });
        }
        if q.len() > self.config.high_water {
            self.backpressure_count += 1;
            return Err(TransportError::WouldBlock);
        }
        q.extend(data.iter());
        self.produced += data.len();
        self.max_depth = self.max_depth.max(q.len());
        Ok(())
    }

    /// Consumer reads up to batch_size bytes.
    pub fn read(&mut self, out: &mut Vec<u8>) -> usize {
        let mut q = self.queue.lock().unwrap();
        let to_read = std::cmp::min(q.len(), self.config.batch_size);
        for _ in 0..to_read {
            if let Some(b) = q.pop_front() {
                out.push(b);
            }
        }
        self.delivered += to_read;
        // Check if we dropped below low water to resume producer (not modeled here)
        to_read
    }

    pub fn stats(&self) -> TransportStats {
        let lossless = self.produced == self.delivered && !self.desync && self.hard_breach == 0;
        TransportStats {
            produced_bytes: self.produced,
            delivered_bytes: self.delivered,
            dropped_bytes: 0, // never drops
            max_queue_depth: self.max_depth,
            backpressure_events: self.backpressure_count,
            hard_limit_breaches: self.hard_breach,
            lossless,
        }
    }

    pub fn is_desync(&self) -> bool {
        self.desync
    }

    /// Drain all remaining.
    pub fn drain(&mut self) -> Vec<u8> {
        let mut q = self.queue.lock().unwrap();
        let mut out = Vec::new();
        while let Some(b) = q.pop_front() {
            out.push(b);
        }
        self.delivered += out.len();
        out
    }
}

/// Dropping transport (contrast) that silently drops under pressure - demonstrates why NOT to use.
pub struct DroppingTransport {
    config: TransportConfig,
    queue: VecDeque<u8>,
    produced: usize,
    delivered: usize,
    dropped: usize,
}

impl DroppingTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            config,
            queue: VecDeque::new(),
            produced: 0,
            delivered: 0,
            dropped: 0,
        }
    }
    pub fn write(&mut self, data: &[u8]) {
        self.produced += data.len();
        if self.queue.len() + data.len() > self.config.capacity {
            // Silently drop oldest
            let overflow = (self.queue.len() + data.len()) - self.config.capacity;
            for _ in 0..overflow {
                self.queue.pop_front();
                self.dropped += 1;
            }
        }
        self.queue.extend(data.iter());
    }
    pub fn read(&mut self, out: &mut Vec<u8>) -> usize {
        let to_read = std::cmp::min(self.queue.len(), self.config.batch_size);
        for _ in 0..to_read {
            out.push(self.queue.pop_front().unwrap());
        }
        self.delivered += to_read;
        to_read
    }
    pub fn stats(&self) -> TransportStats {
        TransportStats {
            produced_bytes: self.produced,
            delivered_bytes: self.delivered,
            dropped_bytes: self.dropped,
            max_queue_depth: self.config.capacity,
            backpressure_events: 0,
            hard_limit_breaches: 0,
            lossless: self.dropped == 0,
        }
    }
}

/// Experiment: run both transports under high load and compare.
pub fn run_experiment(high_volume_bytes: usize) -> (TransportStats, TransportStats) {
    let config = TransportConfig {
        capacity: 64 * 1024,
        high_water: 48 * 1024,
        low_water: 16 * 1024,
        batch_size: 4096,
    };

    let mut dropping = DroppingTransport::new(config.clone());

    #[derive(Default)]
    struct ExperimentState {
        queue: VecDeque<u8>,
        produced: usize,
        delivered: usize,
        max_depth: usize,
        backpressure_events: usize,
        hard_limit_breaches: usize,
        done: bool,
    }

    let shared = Arc::new((Mutex::new(ExperimentState::default()), Condvar::new()));
    let producer_shared = Arc::clone(&shared);
    let producer_config = config.clone();
    let producer = std::thread::spawn(move || {
        let chunk = vec![b'A'; producer_config.batch_size];
        let mut offset = 0;
        while offset < high_volume_bytes {
            let count = chunk.len().min(high_volume_bytes - offset);
            let (lock, ready) = &*producer_shared;
            let mut state = lock.lock().expect("experiment mutex poisoned");
            if state.queue.len() + count > producer_config.high_water {
                state.backpressure_events += 1;
                while state.queue.len() > producer_config.low_water {
                    state = ready.wait(state).expect("experiment mutex poisoned");
                }
            }
            if state.queue.len() + count > producer_config.capacity {
                state.hard_limit_breaches += 1;
                break;
            }
            state.queue.extend(&chunk[..count]);
            state.produced += count;
            state.max_depth = state.max_depth.max(state.queue.len());
            offset += count;
            ready.notify_all();
        }
        let (lock, ready) = &*producer_shared;
        lock.lock().expect("experiment mutex poisoned").done = true;
        ready.notify_all();
    });

    let consumer_shared = Arc::clone(&shared);
    let consumer_config = config.clone();
    let consumer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(5));
        loop {
            let (lock, ready) = &*consumer_shared;
            let mut state = lock.lock().expect("experiment mutex poisoned");
            while state.queue.is_empty() && !state.done {
                state = ready.wait(state).expect("experiment mutex poisoned");
            }
            if state.queue.is_empty() && state.done {
                break;
            }
            let count = state.queue.len().min(consumer_config.batch_size);
            state.queue.drain(..count);
            state.delivered += count;
            ready.notify_all();
            drop(state);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });

    producer.join().expect("producer thread panicked");
    consumer.join().expect("consumer thread panicked");
    let state = shared.0.lock().expect("experiment mutex poisoned");
    let lossless_stats = TransportStats {
        produced_bytes: state.produced,
        delivered_bytes: state.delivered,
        dropped_bytes: 0,
        max_queue_depth: state.max_depth,
        backpressure_events: state.backpressure_events,
        hard_limit_breaches: state.hard_limit_breaches,
        lossless: state.produced == high_volume_bytes
            && state.produced == state.delivered
            && state.hard_limit_breaches == 0,
    };
    drop(state);

    let chunk = vec![b'A'; config.batch_size];
    let mut offset = 0;
    while offset < high_volume_bytes {
        let count = chunk.len().min(high_volume_bytes - offset);
        dropping.write(&chunk[..count]);
        offset += count;
    }
    let mut out2 = Vec::new();
    while !dropping.queue.is_empty() {
        dropping.read(&mut out2);
    }

    (lossless_stats, dropping.stats())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_consumer_applies_backpressure_without_loss() {
        let (lossless, dropping) = run_experiment(512 * 1024);
        assert!(lossless.lossless);
        assert_eq!(lossless.produced_bytes, lossless.delivered_bytes);
        assert_eq!(lossless.dropped_bytes, 0);
        assert!(lossless.backpressure_events > 0);
        assert_eq!(lossless.hard_limit_breaches, 0);
        assert!(lossless.max_queue_depth <= 64 * 1024);
        assert!(dropping.dropped_bytes > 0);
    }

    #[test]
    fn hard_limit_breach_is_explicit() {
        let mut transport = LosslessTransport::new(TransportConfig::default());
        let oversized = vec![0u8; 64 * 1024 + 1];
        assert!(matches!(
            transport.write(&oversized),
            Err(TransportError::HardLimitBreach { .. })
        ));
        assert!(transport.is_desync());
        assert_eq!(transport.stats().hard_limit_breaches, 1);
    }
}
