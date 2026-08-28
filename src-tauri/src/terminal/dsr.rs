//! Stateful DSR/CPR detector for ConPTY startup handling.
//!
//! portable-pty 0.9.0 on Windows may surface `ESC[6n` during ConPTY startup
//! (device status report request). The detector must be STATEFUL across read
//! boundaries — e.g. `ESC [` in one chunk and `6n` in the next must still be
//! detected. Duplicate CPR responses must be avoided: exactly once per complete
//! sequence.
//!
//! Production CPR response uses a bounded, deterministic position derived from
//! the current PTY size when available. When no renderer is attached at startup
//! we still respond promptly (fail-closed timeout guard is owned by the session
//! reader) so the child does not deadlock. The response `ESC[<row>;<col>R`
//! with `row`/`col` reflecting the configured PTY size is safe because:
//! - xterm.js handles CPR as a normal cursor-position report; unsolicited CPR
//!   without a preceding DSR is ignored by VT state machines or treated as
//!   inert; sending a CPR only when DSR was observed prevents spurious reports.
//! - ConPTY's DSR is emitted once per pseudoconsole creation; our stateful
//!   detector consumes it exactly once, regardless of split point.
//! - If a real xterm is attached, its own CPR handling will process the
//!   response that the PTY child consumes; no duplicate is emitted because we
//!   track `handled` and reset the buffer after each complete match.
//! - The fallback coordinates (24,80) match the default PTY size we open, so
//!   any child querying cursor position early receives a consistent answer.
//!
//! References: vercel/turborepo#11816, wezterm#6783.

/// Stateful detector for `ESC[6n` (DSR 6n) across arbitrary read splits.
#[derive(Debug, Default, Clone)]
pub struct DsrDetector {
    /// Tail buffer retaining at most the longest prefix of `ESC[6n` that could
    /// still become a full sequence when combined with the next chunk.
    tail: Vec<u8>,
    /// Number of complete DSR requests observed so far.
    count: usize,
}

impl DsrDetector {
    pub fn new() -> Self {
        Self {
            tail: Vec::with_capacity(8),
            count: 0,
        }
    }

    /// Feed raw bytes from the PTY reader. Returns the number of *complete*
    /// `ESC[6n` sequences detected in this chunk (including those completed
    /// by combining tail + chunk). The detector updates internal tail for the
    /// next call.
    ///
    /// Handles splits:
    /// - `ESC` + `[6n`
    /// - `ESC[` + `6n`
    /// - `ESC[6` + `n`
    /// - and any other 1..3 byte split, plus interleaved unrelated data.
    pub fn feed(&mut self, data: &[u8]) -> usize {
        if data.is_empty() {
            return 0;
        }

        // Combine tail + new data for scanning, then retain new tail.
        let mut combined = Vec::with_capacity(self.tail.len() + data.len());
        combined.extend_from_slice(&self.tail);
        combined.extend_from_slice(data);

        let mut found = 0usize;
        let mut i = 0;
        while i + 3 < combined.len() || i + 4 <= combined.len() {
            // Search for ESC (0x1b) followed by '[' '6' 'n'
            if combined[i] == 0x1b
                && i + 3 < combined.len()
                && combined[i + 1] == b'['
                && combined[i + 2] == b'6'
                && combined[i + 3] == b'n'
            {
                found += 1;
                i += 4;
                continue;
            }
            // Also handle CSI with optional '?' prefix? No — DSR is exactly ESC[6n per ConPTY.
            // Unknown CSI should not be counted.
            i += 1;
        }

        self.count += found;

        // Retain at most 3 trailing bytes that could be a prefix of ESC[6n.
        // Possible prefixes: ESC, ESC[, ESC[6
        self.tail.clear();
        let max_keep = 3usize;
        // Simpler: keep last up to 3 bytes of combined, but filter to only
        // plausible prefixes of ESC[6n.
        let suffix_len = std::cmp::min(max_keep, combined.len());
        let suffix = &combined[combined.len() - suffix_len..];

        // Find longest suffix that is a prefix of ESC[6n
        let target = [0x1b, b'[', b'6', b'n'];
        for keep in (1..=suffix.len()).rev() {
            let candidate = &suffix[suffix.len() - keep..];
            if target.starts_with(candidate) {
                self.tail.extend_from_slice(candidate);
                break;
            }
            // Also handle case where suffix contains earlier ESC that could still complete
            // e.g. suffix = [x, ESC, '['] — the tail should be ESC[
            // Our scan for prefix already covers this; but if suffix is e.g. [ESC, '[', 'X']
            // the longest valid prefix suffix is just ESC? Actually 'ESC[' is prefix, but suffix
            // ending with 'X' not valid. So we should only keep if suffix tail is valid prefix.
            // Above loop handles it: if suffix tail's suffix is valid prefix, keep.
        }

        found
    }

    pub fn total(&self) -> usize {
        self.count
    }

    pub fn reset(&mut self) {
        self.tail.clear();
        self.count = 0;
    }
}

/// Format a CPR response for the given rows/cols. Safe fallback is 24;80.
pub fn cpr_response(rows: u16, cols: u16) -> Vec<u8> {
    let r = if rows == 0 { 24 } else { rows };
    let c = if cols == 0 { 80 } else { cols };
    format!("\x1b[{r};{c}R").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_in_one_read() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"hello\x1b[6nworld"), 1);
    }

    #[test]
    fn split_esc_bracket() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"\x1b"), 0);
        assert_eq!(d.feed(b"[6n"), 1);
    }

    #[test]
    fn split_esc_bracket_6() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"\x1b["), 0);
        assert_eq!(d.feed(b"6n"), 1);
    }

    #[test]
    fn split_esc6_n() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"\x1b[6"), 0);
        assert_eq!(d.feed(b"n"), 1);
    }

    #[test]
    fn multiple_requests() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"\x1b[6n foo \x1b[6n"), 2);
    }

    #[test]
    fn unrelated_csi_not_counted() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"\x1b[31m red \x1b[0m"), 0);
        assert_eq!(d.feed(b"\x1b[?1049h"), 0);
    }

    #[test]
    fn partial_tail_across_many_chunks() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"a"), 0);
        assert_eq!(d.feed(b"\x1b"), 0);
        assert_eq!(d.feed(b"["), 0);
        assert_eq!(d.feed(b"6"), 0);
        assert_eq!(d.feed(b"n"), 1);
        // Next chunk with unrelated data should not double count
        assert_eq!(d.feed(b"more"), 0);
    }

    #[test]
    fn no_duplicate_on_same_bytes_replayed() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"\x1b[6n"), 1);
        // Feeding same again counts as new (it's a new occurrence in stream)
        assert_eq!(d.feed(b"\x1b[6n"), 1);
        assert_eq!(d.total(), 2);
    }

    #[test]
    fn no_false_positive_esc6_without_bracket() {
        let mut d = DsrDetector::new();
        assert_eq!(d.feed(b"\x1b6n"), 0);
        assert_eq!(d.feed(b"ESC[6n literal"), 0);
    }

    #[test]
    fn split_with_interleaved_bytes() {
        let mut d = DsrDetector::new();
        // ESC in one, unrelated, then [6n — should not combine across unrelated
        assert_eq!(d.feed(b"\x1b"), 0);
        assert_eq!(d.feed(b"hello"), 0);
        // tail after "hello" should have been cleared because "hello" tail is not ESC prefix
        // So [6n alone without ESC should not count
        assert_eq!(d.feed(b"[6n"), 0);
        // But ESC[6n together should count
        assert_eq!(d.feed(b"\x1b[6n"), 1);
    }

    #[test]
    fn cpr_response_safe() {
        assert_eq!(cpr_response(24, 80), b"\x1b[24;80R");
        assert_eq!(cpr_response(0, 0), b"\x1b[24;80R");
        assert_eq!(cpr_response(40, 120), b"\x1b[40;120R");
    }
}
