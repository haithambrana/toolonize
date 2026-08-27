//! Bounded LOSSLESS transport experiment.
//! This validates the design for M3: bounded batching with backpressure and no silent drop.

use crossbeam_channel::{bounded, Receiver, Sender};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    // Real bounded experiment: 64 KiB cap, 48 KiB high, 16 KiB low, batch 4 KiB, slow consumer
    let config = TransportConfig {
        capacity: 64 * 1024,
        high_water: 48 * 1024,
        low_water: 16 * 1024,
        batch_size: 4096,
    };

    let mut lossless = LosslessTransport::new(config.clone());
    let mut dropping = DroppingTransport::new(config.clone());

    // Producer is fast, consumer is deliberately slow (drains every 20*batch)
    let pattern = b"TOOLONIZE_LOSSLESS_TEST_PATTERN_0123456789_ABCDEFGHIJ_";
    let mut produced = 0;
    while produced < high_volume_bytes {
        let chunk = &pattern[..std::cmp::min(pattern.len(), high_volume_bytes - produced)];
        // Lossless: handle WouldBlock by draining and retrying (simulating blocking)
        let mut res = lossless.write(chunk);
        if let Err(TransportError::WouldBlock) = res {
            // Backpressure: drain to low_water then retry
            let mut out = Vec::new();
            lossless.read(&mut out);
            res = lossless.write(chunk);
        }
        if res.is_err() {
            // Hard breach - drain and retry once more
            let mut out = Vec::new();
            lossless.read(&mut out);
            let _ = lossless.write(chunk);
        }
        dropping.write(chunk);
        produced += chunk.len();
        // Very slow consumer: only drain every 20 batches
        if produced % (config.batch_size * 20) == 0 {
            let mut out = Vec::new();
            lossless.read(&mut out);
            let mut out2 = Vec::new();
            dropping.read(&mut out2);
            // Simulate WebView being slower than PTY
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    // Final drain
    let mut out = Vec::new();
    while lossless.queue.lock().unwrap().len() > 0 {
        lossless.read(&mut out);
    }
    let mut out2 = Vec::new();
    while dropping.queue.len() > 0 {
        dropping.read(&mut out2);
    }

    (lossless.stats(), dropping.stats())
}
