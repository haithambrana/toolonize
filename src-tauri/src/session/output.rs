//! Lossless per-session output log (NFR-002, ADR-004 mitigation 3).
//!
//! Every byte the PTY produces is appended with a monotonically increasing
//! absolute offset. Attached views read by offset and acknowledge what xterm.js
//! has actually written, so delivery is exact and resumable. While a view is
//! attached nothing it has not acknowledged is ever trimmed; instead the PTY
//! reader blocks (backpressure). Without a view, the log keeps a bounded
//! replay window; anything older is trimmed and a later attach is told so
//! explicitly (`trimmed`), never silently.

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowLimits {
    /// Bytes retained for replay when no view holds them.
    pub retention: usize,
    /// Unacknowledged bytes at which the PTY reader pauses.
    pub high_water: usize,
    /// Unacknowledged bytes at which a paused reader resumes.
    pub low_water: usize,
    /// Maximum bytes in flight to a view before acknowledgement.
    pub window: usize,
    /// Maximum bytes in one IPC frame.
    pub batch: usize,
}

impl Default for FlowLimits {
    fn default() -> Self {
        // Informed by the M2 transport spike (64 KiB capacity sustained a
        // slow consumer with zero breaches); larger values here only buy
        // throughput headroom while staying far below NFR-003 memory budgets.
        Self {
            retention: 4 * 1024 * 1024,
            high_water: 2 * 1024 * 1024,
            low_water: 1024 * 1024,
            window: 1024 * 1024,
            batch: 256 * 1024,
        }
    }
}

impl FlowLimits {
    pub fn validate(&self) -> bool {
        self.low_water < self.high_water
            && self.high_water <= self.retention
            && self.window <= self.high_water
            && self.batch > 0
            && self.batch <= self.window
    }
}

#[derive(Debug)]
pub struct OutputLog {
    bytes: VecDeque<u8>,
    base: u64,
    limits: FlowLimits,
}

impl OutputLog {
    pub fn new(limits: FlowLimits) -> Self {
        debug_assert!(limits.validate());
        Self {
            bytes: VecDeque::new(),
            base: 0,
            limits,
        }
    }

    pub fn limits(&self) -> FlowLimits {
        self.limits
    }

    pub fn base(&self) -> u64 {
        self.base
    }

    pub fn end(&self) -> u64 {
        self.base + self.bytes.len() as u64
    }

    pub fn retained(&self) -> usize {
        self.bytes.len()
    }

    pub fn append(&mut self, data: &[u8]) {
        self.bytes.extend(data);
    }

    /// Trim to the retention window, never past `protect_from`.
    pub fn trim(&mut self, protect_from: Option<u64>) {
        let excess = self.bytes.len().saturating_sub(self.limits.retention) as u64;
        if excess == 0 {
            return;
        }
        let mut new_base = self.base + excess;
        if let Some(protected) = protect_from {
            new_base = new_base.min(protected.max(self.base));
        }
        let drop = (new_base - self.base) as usize;
        self.bytes.drain(..drop);
        self.base = new_base;
    }

    /// Copy up to `max` bytes starting at `offset` (clamped to what exists).
    pub fn read(&self, offset: u64, max: usize) -> (u64, Vec<u8>) {
        let start = offset.clamp(self.base, self.end());
        let from = (start - self.base) as usize;
        let to = (from + max).min(self.bytes.len());
        let (a, b) = self.bytes.as_slices();
        let mut out = Vec::with_capacity(to - from);
        if from < a.len() {
            out.extend_from_slice(&a[from..to.min(a.len())]);
        }
        if to > a.len() {
            out.extend_from_slice(&b[from.saturating_sub(a.len())..to - a.len()]);
        }
        (start, out)
    }
}

/// Encode an output frame: little-endian u64 start offset, then the bytes.
pub fn encode_frame(offset: u64, data: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(8 + data.len());
    frame.extend_from_slice(&offset.to_le_bytes());
    frame.extend_from_slice(data);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> FlowLimits {
        FlowLimits {
            retention: 16,
            high_water: 8,
            low_water: 4,
            window: 8,
            batch: 4,
        }
    }

    #[test]
    fn default_limits_are_consistent() {
        assert!(FlowLimits::default().validate());
        assert!(small().validate());
    }

    #[test]
    fn append_and_read_by_offset() {
        let mut log = OutputLog::new(small());
        log.append(b"hello world");
        assert_eq!(log.end(), 11);
        assert_eq!(log.read(6, 100), (6, b"world".to_vec()));
        assert_eq!(log.read(0, 5), (0, b"hello".to_vec()));
        assert_eq!(log.read(50, 5), (11, Vec::new()));
    }

    #[test]
    fn trims_only_to_retention_and_respects_protection() {
        let mut log = OutputLog::new(small());
        log.append(&[b'a'; 20]);
        log.trim(Some(2));
        assert_eq!(log.base(), 2, "protected offset must not be trimmed");
        log.trim(None);
        assert_eq!(log.base(), 4);
        assert_eq!(log.retained(), 16);
        let (start, data) = log.read(0, 100);
        assert_eq!(start, 4, "reads before base are clamped, caller sees trim");
        assert_eq!(data.len(), 16);
    }

    #[test]
    fn read_spanning_ring_wrap() {
        let mut log = OutputLog::new(small());
        log.append(b"0123456789abcdef");
        log.append(b"ghij");
        log.trim(None);
        let (start, data) = log.read(log.base(), 100);
        assert_eq!(start, 4);
        assert_eq!(data, b"456789abcdefghij");
    }

    #[test]
    fn frame_encoding() {
        let frame = encode_frame(258, b"xy");
        assert_eq!(&frame[..8], &258u64.to_le_bytes());
        assert_eq!(&frame[8..], b"xy");
    }
}
