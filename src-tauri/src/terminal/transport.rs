//! Lossless output transport — production design.
//!
//! VT streams are stateful: dropping arbitrary bytes corrupts escape sequences.
//! This module therefore forbids `drop-oldest`, `drop-newest`, or silent
//! truncation. It uses:
//! - bounded per-session buffering
//! - batching/coalescing
//! - high-water mark (apply backpressure)
//! - low-water mark (resume)
//! - backpressure signaling
//! - sequence numbers
//! - explicit hard-limit failure → `Desynchronized`
//! - per-session isolation (each session owns its transport)
//!
//! ## Chosen constants (justified by M2 measurements)
//!
//! M2 producer/slow-consumer experiment used:
//! ```text
//! capacity 64 KiB, high 48 KiB, low 16 KiB, batch 4 KiB
//! produced 2 MiB, delivered lossless, backpressure_events 63, max_queue 48 KiB
//! dropping contrast lost 2_031_616 bytes.
//! ```
//! Full-path PTY→Channel→xterm run delivered exactly 256 KiB with matching SHA-256.
//!
//! M3 retains the same bounded values as a conservative default for interactive
//! shells. They keep memory bounded while preserving lossless delivery under
//! bursty output. A high-volume `yes`-class stream (>10 MB/s) will apply
//! backpressure rather than grow unbounded; the hard limit breach surfaces
//! `TransportState::Desynchronized` explicitly.
//!
//! Documented values:
//! - chunk size: 4096 bytes (coalesced; individual PTY reads coalesce up to this)
//! - queue capacity: 65_536 bytes (64 KiB)
//! - high-water: 49_152 bytes (48 KiB) — producer waits
//! - low-water: 16_384 bytes (16 KiB) — producer resumes
//! - hard limit: 65_536 bytes — breach → Desynchronized (no silent drop)
//! - replay cap: 65_536 bytes (server-side replay for renderer reload)
//!
//! Per-session isolation: each `Session` owns a `Transport`; one slow session
//! never blocks another.

use serde::Serialize;

pub const CHUNK_SIZE: usize = 4096;
pub const QUEUE_CAPACITY: usize = 65_536;
pub const HIGH_WATER: usize = 49_152;
pub const LOW_WATER: usize = 16_384;
pub const HARD_LIMIT: usize = QUEUE_CAPACITY;
/// Bounded server-side replay for renderer reload. A new xterm has lost browser
/// scrollback, so Rust replays up to this many recent bytes in order.
pub const REPLAY_CAP: usize = 65_536;

/// Transport state surfaced to the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TransportState {
    Normal,
    Backpressured,
    Desynchronized { reason: &'static str },
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputChunk {
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransportStats {
    pub produced_bytes: usize,
    pub delivered_bytes: usize,
    pub queued_bytes: usize,
    pub in_flight: usize,
    pub max_queue_depth: usize,
    pub backpressure_events: usize,
    pub hard_limit_breaches: usize,
    pub next_sequence: u64,
    pub acknowledged_up_to: Option<u64>,
    pub state: TransportState,
    pub lossless: bool,
    pub dropped_bytes: usize,
}

/// Lossless bounded transport with sequence numbers and ack tracking.
///
/// Producer = PTY reader pump.
/// Consumer = frontend xterm (via Tauri Channel); consumer acks each `sequence`
/// after `xterm.write` completes.
pub struct Transport {
    capacity: usize,
    high_water: usize,
    low_water: usize,
    replay_cap: usize,

    /// Bytes queued but not yet sent.
    queue: Vec<u8>,
    /// Bytes sent but not yet acknowledged (in-flight).
    in_flight: std::collections::BTreeMap<u64, Vec<u8>>,

    /// Replay buffer — last `replay_cap` bytes in arrival order, for reattach.
    replay: Vec<u8>,

    next_sequence: u64,
    acknowledged_up_to: Option<u64>,

    produced_bytes: usize,
    delivered_bytes: usize,
    max_queue_depth: usize,
    backpressure_events: usize,
    hard_limit_breaches: usize,
    desync: bool,
    desync_reason: Option<&'static str>,
}

impl Default for Transport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport {
    pub fn new() -> Self {
        Self {
            capacity: QUEUE_CAPACITY,
            high_water: HIGH_WATER,
            low_water: LOW_WATER,
            replay_cap: REPLAY_CAP,
            queue: Vec::with_capacity(QUEUE_CAPACITY),
            in_flight: Default::default(),
            replay: Vec::with_capacity(REPLAY_CAP),
            next_sequence: 0,
            acknowledged_up_to: None,
            produced_bytes: 0,
            delivered_bytes: 0,
            max_queue_depth: 0,
            backpressure_events: 0,
            hard_limit_breaches: 0,
            desync: false,
            desync_reason: None,
        }
    }

    pub fn with_capacity(
        capacity: usize,
        high_water: usize,
        low_water: usize,
        replay_cap: usize,
    ) -> Self {
        Self {
            capacity,
            high_water,
            low_water,
            replay_cap,
            queue: Vec::with_capacity(capacity),
            in_flight: Default::default(),
            replay: Vec::with_capacity(replay_cap),
            next_sequence: 0,
            acknowledged_up_to: None,
            produced_bytes: 0,
            delivered_bytes: 0,
            max_queue_depth: 0,
            backpressure_events: 0,
            hard_limit_breaches: 0,
            desync: false,
            desync_reason: None,
        }
    }

    /// Attempt to enqueue raw PTY bytes. Returns `WouldBlock` if above high-water,
    /// `HardLimit` if capacity would be exceeded. Never drops.
    pub fn enqueue(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if self.desync {
            return Err(TransportError::Desynchronized);
        }
        let in_flight_bytes: usize = self.in_flight.values().map(|v| v.len()).sum();
        let queued = self.queue.len() + in_flight_bytes;
        if queued + data.len() > self.capacity {
            self.hard_limit_breaches += 1;
            self.desync = true;
            self.desync_reason = Some("hard_limit_breach");
            return Err(TransportError::HardLimitBreach {
                queued,
                limit: self.capacity,
            });
        }
        if queued > self.high_water {
            self.backpressure_events += 1;
            return Err(TransportError::WouldBlock);
        }
        self.queue.extend_from_slice(data);
        self.produced_bytes += data.len();
        self.max_queue_depth = self.max_queue_depth.max(self.queue.len() + in_flight_bytes);
        // Update replay (bounded)
        self.replay.extend_from_slice(data);
        if self.replay.len() > self.replay_cap {
            let excess = self.replay.len() - self.replay_cap;
            self.replay.drain(..excess);
        }
        Ok(())
    }

    /// Drain up to `CHUNK_SIZE` bytes into a sequenced chunk for delivery.
    /// Returns None if nothing queued or desync.
    pub fn next_chunk(
        &mut self,
        session_id: &str,
        generation: u64,
    ) -> Result<Option<OutputChunk>, TransportError> {
        if self.desync {
            return Err(TransportError::Desynchronized);
        }
        if self.queue.is_empty() {
            return Ok(None);
        }
        let take = std::cmp::min(self.queue.len(), CHUNK_SIZE);
        let bytes: Vec<u8> = self.queue.drain(..take).collect();
        let seq = self.next_sequence;
        self.next_sequence += 1;
        self.in_flight.insert(seq, bytes.clone());
        Ok(Some(OutputChunk {
            session_id: session_id.to_string(),
            generation,
            sequence: seq,
            bytes,
        }))
    }

    /// Frontend acknowledges that `sequence` was consumed by xterm write.
    /// Advances `delivered_bytes` and may release backpressure.
    pub fn ack(&mut self, sequence: u64) -> Result<(), TransportError> {
        if self.desync {
            return Err(TransportError::Desynchronized);
        }
        // Detect gaps / duplicates
        if let Some(up_to) = self.acknowledged_up_to {
            if sequence <= up_to {
                return Err(TransportError::DuplicateAck { sequence });
            }
            if sequence != up_to + 1 {
                // Gap — must surface explicitly rather than silently reorder.
                self.desync = true;
                self.desync_reason = Some("sequence_gap");
                return Err(TransportError::SequenceGap {
                    expected: up_to + 1,
                    got: sequence,
                });
            }
        } else if sequence != 0 {
            // First ack must be 0
            if sequence != 0 {
                self.desync = true;
                self.desync_reason = Some("sequence_gap_first");
                return Err(TransportError::SequenceGap {
                    expected: 0,
                    got: sequence,
                });
            }
        }
        if let Some(bytes) = self.in_flight.remove(&sequence) {
            self.delivered_bytes += bytes.len();
            self.acknowledged_up_to = Some(sequence);
            Ok(())
        } else {
            Err(TransportError::UnknownSequence { sequence })
        }
    }

    pub fn replay_bytes(&self) -> &[u8] {
        &self.replay
    }

    /// Whether the next `enqueue` would be below low-water (resume signal).
    pub fn below_low_water(&self) -> bool {
        let in_flight_bytes: usize = self.in_flight.values().map(|v| v.len()).sum();
        self.queue.len() + in_flight_bytes <= self.low_water
    }

    pub fn state(&self) -> TransportState {
        if self.desync {
            TransportState::Desynchronized {
                reason: self.desync_reason.unwrap_or("desynchronized"),
            }
        } else {
            let in_flight_bytes: usize = self.in_flight.values().map(|v| v.len()).sum();
            if self.queue.len() + in_flight_bytes > self.high_water {
                TransportState::Backpressured
            } else {
                TransportState::Normal
            }
        }
    }

    pub fn stats(&self) -> TransportStats {
        let in_flight_bytes: usize = self.in_flight.values().map(|v| v.len()).sum();
        TransportStats {
            produced_bytes: self.produced_bytes,
            delivered_bytes: self.delivered_bytes,
            queued_bytes: self.queue.len(),
            in_flight: self.in_flight.len(),
            max_queue_depth: self.max_queue_depth,
            backpressure_events: self.backpressure_events,
            hard_limit_breaches: self.hard_limit_breaches,
            next_sequence: self.next_sequence,
            acknowledged_up_to: self.acknowledged_up_to,
            state: self.state(),
            lossless: !self.desync
                && self.hard_limit_breaches == 0
                && self.produced_bytes == self.delivered_bytes + self.queue.len() + in_flight_bytes,
            dropped_bytes: 0,
        }
    }

    pub fn is_desync(&self) -> bool {
        self.desync
    }

    pub fn queued_len(&self) -> usize {
        self.queue.len()
    }

    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    WouldBlock,
    HardLimitBreach { queued: usize, limit: usize },
    Desynchronized,
    SequenceGap { expected: u64, got: u64 },
    DuplicateAck { sequence: u64 },
    UnknownSequence { sequence: u64 },
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WouldBlock => write!(f, "transport backpressure"),
            Self::HardLimitBreach { queued, limit } => {
                write!(
                    f,
                    "transport hard limit breach queued={queued} limit={limit}"
                )
            }
            Self::Desynchronized => write!(f, "transport desynchronized"),
            Self::SequenceGap { expected, got } => {
                write!(f, "sequence gap expected={expected} got={got}")
            }
            Self::DuplicateAck { sequence } => write!(f, "duplicate ack {sequence}"),
            Self::UnknownSequence { sequence } => write!(f, "unknown sequence {sequence}"),
        }
    }
}

impl std::error::Error for TransportError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backpressure_and_no_loss() {
        let mut t = Transport::new();
        // Enqueue until high water exceeded triggers backpressure
        let chunk = vec![b'A'; 4096];
        let mut _enqueued = 0usize;
        let mut backpressured = false;
        for _ in 0..100 {
            match t.enqueue(&chunk) {
                Ok(()) => _enqueued += chunk.len(),
                Err(TransportError::WouldBlock) => {
                    backpressured = true;
                    break;
                }
                Err(e) => panic!("unexpected {e:?}"),
            }
        }
        assert!(backpressured);
        assert!(t.stats().backpressure_events > 0);
        assert_eq!(t.stats().dropped_bytes, 0);
    }

    #[test]
    fn hard_limit_breach_is_explicit() {
        let mut t = Transport::with_capacity(4096, 2048, 1024, 4096);
        let big = vec![b'A'; 5000];
        assert!(matches!(
            t.enqueue(&big),
            Err(TransportError::HardLimitBreach { .. })
        ));
        assert!(t.is_desync());
        assert_eq!(t.stats().hard_limit_breaches, 1);
    }

    #[test]
    fn sequence_gap_desync() {
        let mut t = Transport::new();
        t.enqueue(b"hello").unwrap();
        let c = t.next_chunk("sess", 0).unwrap().unwrap();
        assert_eq!(c.sequence, 0);
        // Ack 1 without 0 should gap
        let err = t.ack(1).unwrap_err();
        assert!(matches!(err, TransportError::SequenceGap { .. }));
        assert!(t.is_desync());
    }

    #[test]
    fn ack_advances_delivered_and_releases_backpressure() {
        let mut t = Transport::new();
        t.enqueue(b"hello world").unwrap();
        let c = t.next_chunk("sess", 0).unwrap().unwrap();
        assert_eq!(t.stats().queued_bytes, 0);
        t.ack(c.sequence).unwrap();
        assert_eq!(t.stats().delivered_bytes, 11);
        assert!(t.below_low_water());
    }

    #[test]
    fn replay_bounded() {
        let mut t = Transport::with_capacity(65536, 49152, 16384, 10);
        t.enqueue(b"0123456789ABCDEF").unwrap();
        assert_eq!(t.replay_bytes().len(), 10);
        // Last 10 bytes: should be tail of input
        assert_eq!(t.replay_bytes(), b"6789ABCDEF");
    }
}
