//! Device Status Report (cursor position request) handling.
//!
//! ADR-004 mitigation 1: a `ESC[6n` request split across reads must be
//! retained and answered only once it is complete. ConPTY emits this request
//! at startup and blocks until it is answered, so the core may need to answer
//! before any view is attached. To keep exactly one reply per request, the
//! session counts the requests it answered itself and drops the matching
//! duplicate reply that xterm.js produces later (see [`is_cursor_report`]).

const REQUEST: &[u8] = b"\x1b[6n";

#[derive(Debug, Default, Clone)]
pub struct DsrScanner {
    tail: Vec<u8>,
}

impl DsrScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Count complete DSR requests in `data`, including ones that started in a
    /// previous chunk. Never reports a request twice.
    pub fn scan(&mut self, data: &[u8]) -> usize {
        let mut window = std::mem::take(&mut self.tail);
        window.extend_from_slice(data);
        let mut count = 0;
        let mut index = 0;
        while index + REQUEST.len() <= window.len() {
            if &window[index..index + REQUEST.len()] == REQUEST {
                count += 1;
                index += REQUEST.len();
            } else {
                index += 1;
            }
        }
        // Keep only a suffix that could still be the start of a request.
        let keep_from = (index..window.len())
            .find(|&start| REQUEST.starts_with(&window[start..]))
            .unwrap_or(window.len());
        self.tail = window[keep_from..].to_vec();
        count
    }
}

/// True when `input` is exactly one cursor position report (`ESC[<r>;<c>R`).
pub fn is_cursor_report(input: &[u8]) -> bool {
    let Some(body) = input
        .strip_prefix(b"\x1b[")
        .and_then(|rest| rest.strip_suffix(b"R"))
    else {
        return false;
    };
    let mut parts = body.split(|&b| b == b';');
    let (Some(row), Some(col), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let digits =
        |part: &[u8]| !part.is_empty() && part.len() <= 5 && part.iter().all(u8::is_ascii_digit);
    digits(row) && digits(col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_whole_requests() {
        let mut scanner = DsrScanner::new();
        assert_eq!(scanner.scan(b"hello\x1b[6nworld\x1b[6n"), 2);
        assert_eq!(scanner.scan(b"nothing"), 0);
    }

    #[test]
    fn retains_request_split_across_reads() {
        // T-PTY-004: every split position must be answered exactly once.
        for split in 0..=REQUEST.len() {
            let mut scanner = DsrScanner::new();
            let mut data = b"abc".to_vec();
            data.extend_from_slice(REQUEST);
            data.extend_from_slice(b"xyz");
            let cut = 3 + split;
            let first = scanner.scan(&data[..cut]);
            let second = scanner.scan(&data[cut..]);
            assert_eq!(first + second, 1, "split at {split}");
        }
    }

    #[test]
    fn split_across_three_reads() {
        let mut scanner = DsrScanner::new();
        assert_eq!(scanner.scan(b"\x1b"), 0);
        assert_eq!(scanner.scan(b"["), 0);
        assert_eq!(scanner.scan(b"6"), 0);
        assert_eq!(scanner.scan(b"n"), 1);
    }

    #[test]
    fn near_miss_is_not_a_request() {
        let mut scanner = DsrScanner::new();
        assert_eq!(scanner.scan(b"\x1b[5n\x1b[6m\x1b6n"), 0);
    }

    #[test]
    fn cursor_report_shape() {
        assert!(is_cursor_report(b"\x1b[1;1R"));
        assert!(is_cursor_report(b"\x1b[24;120R"));
        assert!(!is_cursor_report(b"\x1b[1;1Rx"));
        assert!(!is_cursor_report(b"\x1b[;1R"));
        assert!(!is_cursor_report(b"\x1b[1R"));
        assert!(!is_cursor_report(b"ls\r"));
    }
}
