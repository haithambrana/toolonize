//! Session memory (ADR-015): what is needed to continue after the app was
//! closed or the machine lost power — which terminals were running, in which
//! folder and size, the window's place, and the last part of each terminal's
//! output.
//!
//! Saved terminal output is replayed into a new terminal before its first
//! byte of new output. Replayed text must never *do* anything: it is reduced
//! to printable text, line controls and harmless formatting (colors, erase
//! to end of line, cursor moves on the line). Everything that could have an
//! effect — clipboard writes (OSC 52), notifications, titles, working
//! directory reports, device queries whose answers would be typed into the
//! new shell (DSR, DA, DECRQSS ...), full resets, mode changes, and the
//! contents of full-screen (alternate screen) programs — is removed, both
//! when a snapshot is taken and again when it is loaded.

use serde::{Deserialize, Serialize};

pub const INDEX_FORMAT: &str = "toolonize-session-memory";
pub const TERM_FORMAT: &str = "toolonize-terminal-memory";
pub const WINDOW_FORMAT: &str = "toolonize-window";
pub const MEMORY_SCHEMA: u32 = 1;
/// Raw output read from the end of a session's replay buffer per snapshot.
pub const MAX_RAW_TAIL: usize = 1024 * 1024;
/// Saved (cleaned) text per terminal.
pub const MAX_SAVED_BYTES: usize = 256 * 1024;

/// One terminal that was running when the last snapshot was taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningTerminal {
    pub workspace_id: String,
    pub member_id: String,
    pub rows: u16,
    pub cols: u16,
    /// Last known working directory (local shells only).
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryIndex {
    #[serde(default)]
    pub running: Vec<RunningTerminal>,
    /// Set when the app exits normally; false at the next start means the
    /// previous run ended abruptly (crash, power loss, killed).
    #[serde(default)]
    pub clean_exit: bool,
    #[serde(default)]
    pub saved_at: u64,
}

/// The saved output of one terminal (only when content memory is on).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalMemory {
    pub text: String,
    pub saved_at: u64,
    /// Terminal width when saved: the text is shown at this width first so
    /// that lines the shell wrapped come back intact.
    #[serde(default)]
    pub cols: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

/// A monitor's work area (physical pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowState {
    /// The saved window, if at least 100×60 px of its title area is on one
    /// of the current monitors (a monitor may have been unplugged); sizes
    /// are clamped to that monitor.
    pub fn placed_on(&self, monitors: &[Area]) -> Option<WindowState> {
        if self.width < 200 || self.height < 150 {
            return None;
        }
        monitors.iter().find_map(|m| {
            let (mx2, my2) = (m.x as i64 + m.width as i64, m.y as i64 + m.height as i64);
            let (x, y) = (self.x as i64, self.y as i64);
            let visible_w = (x + self.width as i64).min(mx2) - x.max(m.x as i64);
            let title_visible = y >= m.y as i64 - 10 && y + 60 <= my2;
            (visible_w >= 100 && title_visible).then(|| WindowState {
                width: self.width.min(m.width),
                height: self.height.min(m.height),
                ..*self
            })
        })
    }
}

/// Charset check for ids used in file names (generated `ws_<hex>`/`m_<hex>`).
pub fn file_safe(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn term_store_name(workspace_id: &str, member_id: &str) -> Option<String> {
    (file_safe(workspace_id) && file_safe(member_id))
        .then(|| format!("term-{workspace_id}-{member_id}"))
}

fn is_alt_screen(params: &str) -> bool {
    params
        .strip_prefix('?')
        .is_some_and(|p| p.split(';').any(|n| matches!(n, "47" | "1047" | "1049")))
}

/// CSI sequences that only format text or move within what was printed.
fn csi_allowed(params: &str, intermediates: &str, final_byte: char) -> bool {
    intermediates.is_empty()
        && params
            .chars()
            .all(|c| c.is_ascii_digit() || c == ';' || c == ':')
        && matches!(
            final_byte,
            'm' | 'K' | 'A' | 'B' | 'C' | 'D' | 'G' | 'X' | 'P' | '@'
        )
}

/// Reduce raw terminal output to text that is safe to replay (see module
/// docs). Content shown on the alternate screen is dropped.
pub fn sanitize(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut alt = false;
    // If the tail starts inside a full-screen program, everything before
    // its exit is alternate-screen content.
    let mut pending_alt_exit_check = true;
    let mut i = 0;
    let n = chars.len();
    while i < n {
        let c = chars[i];
        if c == '\u{1b}' {
            let Some(&next) = chars.get(i + 1) else {
                break;
            };
            match next {
                '[' => {
                    let mut j = i + 2;
                    let mut params = String::new();
                    let mut inter = String::new();
                    while j < n && ('\u{30}'..='\u{3f}').contains(&chars[j]) {
                        params.push(chars[j]);
                        j += 1;
                    }
                    while j < n && ('\u{20}'..='\u{2f}').contains(&chars[j]) {
                        inter.push(chars[j]);
                        j += 1;
                    }
                    let Some(&fin) = chars.get(j) else {
                        break;
                    };
                    if !('\u{40}'..='\u{7e}').contains(&fin) {
                        // Malformed: drop the introducer only.
                        i += 2;
                        continue;
                    }
                    if is_alt_screen(&params) && (fin == 'h' || fin == 'l') {
                        if fin == 'h' {
                            alt = true;
                        } else {
                            if pending_alt_exit_check && !alt {
                                out.clear();
                            }
                            alt = false;
                        }
                        pending_alt_exit_check = false;
                    } else if !alt && csi_allowed(&params, &inter, fin) {
                        out.push_str("\u{1b}[");
                        out.push_str(&params);
                        out.push(fin);
                    }
                    i = j + 1;
                }
                // OSC, DCS, SOS, PM, APC: skip the whole string.
                ']' | 'P' | 'X' | '^' | '_' => {
                    let mut j = i + 2;
                    while j < n {
                        if chars[j] == '\u{7}' && next == ']' {
                            j += 1;
                            break;
                        }
                        if chars[j] == '\u{1b}' && chars.get(j + 1) == Some(&'\\') {
                            j += 2;
                            break;
                        }
                        j += 1;
                    }
                    i = j;
                }
                // Charset designation: ESC ( B and friends (one more byte).
                '(' | ')' | '*' | '+' | '-' | '.' | '/' | '#' | '%' | ' ' => i += 3,
                // Any other two-byte escape (save/restore cursor, RIS, keypad).
                _ => i += 2,
            }
            continue;
        }
        if alt {
            i += 1;
            continue;
        }
        match c {
            '\n' | '\r' | '\t' | '\u{8}' => out.push(c),
            c if c.is_control() => {} // other C0, DEL, C1 (incl. 8-bit CSI/OSC)
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// The last part of a session's raw output as text safe to replay, at most
/// [`MAX_SAVED_BYTES`], starting at a line boundary.
pub fn snapshot_text(raw: &[u8]) -> String {
    let start = if raw.len() >= MAX_RAW_TAIL {
        // Started mid-stream: begin at the next line.
        raw.iter()
            .position(|&b| b == b'\n')
            .map_or(raw.len(), |p| p + 1)
    } else {
        0
    };
    let text = sanitize(&String::from_utf8_lossy(&raw[start..]));
    trim_to_limit(text)
}

fn trim_to_limit(text: String) -> String {
    if text.len() <= MAX_SAVED_BYTES {
        return text;
    }
    let mut cut = text.len() - MAX_SAVED_BYTES;
    while !text.is_char_boundary(cut) {
        cut += 1;
    }
    let from = text[cut..].find('\n').map_or(text.len(), |p| cut + p + 1);
    text[from..].to_string()
}

/// What a resumed terminal shows before its new output: the saved text
/// (cleaned again — the file is outside our control once written), then a
/// dim divider line.
pub fn prelude(saved: &TerminalMemory) -> Vec<u8> {
    let text = trim_to_limit(sanitize(&saved.text));
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut out = String::from("\u{1b}[0m");
    out.push_str(&text.replace("\r\n", "\n").replace('\n', "\r\n"));
    if !out.ends_with("\r\n") {
        out.push_str("\r\n");
    }
    out.push_str("\u{1b}[0m\u{1b}[2m── restored from your previous session ──\u{1b}[0m\r\n");
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every escape left after sanitizing is `ESC [ <digits;:> <allowed>`.
    fn only_safe_escapes(s: &str) {
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            assert!(
                !c.is_control() || matches!(c, '\n' | '\r' | '\t' | '\u{8}' | '\u{1b}'),
                "control {:?} left in {s:?}",
                c
            );
            if c == '\u{1b}' {
                assert_eq!(chars.get(i + 1), Some(&'['), "{s:?}");
                let mut j = i + 2;
                while chars[j].is_ascii_digit() || chars[j] == ';' || chars[j] == ':' {
                    j += 1;
                }
                assert!("mKABCDGXP@".contains(chars[j]), "{s:?}");
                i = j;
            }
            i += 1;
        }
    }

    #[test]
    fn keeps_text_colors_and_line_controls() {
        let raw = "\x1b[1;32muser@box\x1b[0m:~$ ls\r\nfile.txt\tdir\r\nprogress 50%\r\x1b[Kdone\x1b[2C!\n";
        assert_eq!(sanitize(raw), raw);
        assert_eq!(sanitize("مرحبا 👋\r\n"), "مرحبا 👋\r\n");
    }

    #[test]
    fn removes_everything_that_could_act() {
        let raw = concat!(
            "a\x1b]52;c;aGVsbG8=\x07",                 // clipboard write
            "b\x1b]0;title\x1b\\",                     // window title (ST)
            "c\x1b]7;file://host/tmp\x07",             // working directory
            "d\x1b]9;done\x07",                        // notification
            "e\x1b]777;notify;x;y\x07",                // notification
            "f\x1b]8;;https://x\x07link\x1b]8;;\x07",  // hyperlink (text kept)
            "g\x1b[6n",                                // cursor position query
            "h\x1b[c\x1b[>c\x1b[=c",                   // device attributes queries
            "i\x1bP$qm\x1b\\",                         // DECRQSS
            "j\x1b[?2004h\x1b[?1h\x1b=",               // modes / keypad
            "k\x1bc",                                  // full reset
            "l\x1b[2J\x1b[3J\x1b[H",                   // clears and absolute moves
            "m\x07\x00\x7f\u{9b}6n\u{9d}52;c;x\u{9c}", // BEL, NUL, DEL, C1
            "n\x1b[18t\x1b[?1$p\x1b[5n",               // window/mode/status reports
            "o\x1b_apc\x1b\\\x1b^pm\x1b\\\x1bXsos\x1b\\",
            "p\x1b(0q\x1b(B\x1b7\x1b8",
        );
        let clean = sanitize(raw);
        only_safe_escapes(&clean);
        assert_eq!(clean, "abcdeflinkghijklm6n52;c;xnopq", "{clean:?}");
    }

    #[test]
    fn drops_full_screen_program_content() {
        let raw = "$ vim notes\r\n\x1b[?1049h\x1b[Hsecret buffer\x1b[?1049l$ echo ok\r\nok\r\n";
        assert_eq!(sanitize(raw), "$ vim notes\r\n$ echo ok\r\nok\r\n");
        // Tail that starts inside a full-screen program.
        let raw = "inside htop\x1b[?1049l$ after\r\n";
        assert_eq!(sanitize(raw), "$ after\r\n");
        // Still inside at the end: nothing after the switch.
        assert_eq!(sanitize("$ top\r\n\x1b[?1049hlive"), "$ top\r\n");
        assert_eq!(sanitize("x\x1b[?47hy\x1b[?47lz"), "xz");
    }

    #[test]
    fn truncated_sequences_never_leak() {
        for raw in ["abc\x1b", "abc\x1b[12", "abc\x1b]52;c;aGVs", "abc\x1bP$q"] {
            let s = sanitize(raw);
            only_safe_escapes(&s);
            assert_eq!(s, "abc", "{raw:?}");
        }
    }

    #[test]
    fn fuzzed_input_only_ever_yields_safe_escapes() {
        // Deterministic pseudo-random bytes biased towards escape material.
        let alphabet: Vec<char> = "\x1b[]P_^X;?0123456789mhlnc$q\x07\\\r\n abc\u{9b}\u{9d}"
            .chars()
            .collect();
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..2000 {
            let mut s = String::new();
            for _ in 0..64 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                s.push(alphabet[(seed % alphabet.len() as u64) as usize]);
            }
            only_safe_escapes(&sanitize(&s));
        }
    }

    #[test]
    fn snapshots_are_bounded_and_start_on_a_line() {
        let line = "0123456789".repeat(10) + "\r\n";
        let raw = line.repeat(20_000); // ~2 MiB
        let tail = &raw.as_bytes()[raw.len() - MAX_RAW_TAIL..];
        let text = snapshot_text(tail);
        assert!(text.len() <= MAX_SAVED_BYTES);
        assert!(text.starts_with("0123"), "{:?}", &text[..20]);
        // Multi-byte characters at the cut are not split.
        let wide = "ث".repeat(200_000) + "\n" + &"ج".repeat(10);
        let t = trim_to_limit(wide);
        assert_eq!(t, "ج".repeat(10));
        assert_eq!(snapshot_text(b"short\r\n"), "short\r\n");
    }

    #[test]
    fn prelude_is_cleaned_again_and_marked() {
        let saved = TerminalMemory {
            text: "$ make\nok\x1b]52;c;evil\x07\x1b[6n".into(),
            saved_at: 1,
            cols: 80,
        };
        let p = String::from_utf8(prelude(&saved)).unwrap();
        assert!(p.starts_with("\x1b[0m$ make\r\nok\r\n"), "{p:?}");
        assert!(!p.contains("52;") && !p.contains("[6n"));
        assert!(p.contains("restored from your previous session"));
        assert!(p.ends_with("\r\n"));
        let empty = TerminalMemory {
            text: " \r\n".into(),
            saved_at: 1,
            cols: 80,
        };
        assert!(prelude(&empty).is_empty());
    }

    #[test]
    fn window_must_be_visible_on_a_monitor() {
        let m = [Area {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }];
        let w = WindowState {
            x: 100,
            y: 50,
            width: 1200,
            height: 800,
            maximized: false,
        };
        assert_eq!(w.placed_on(&m), Some(w));
        // On a monitor that is gone.
        let gone = WindowState { x: 2500, ..w };
        assert_eq!(gone.placed_on(&m), None);
        // Bigger than the monitor: clamped.
        let big = WindowState {
            width: 4000,
            height: 3000,
            x: 0,
            y: 0,
            ..w
        };
        assert_eq!(big.placed_on(&m).unwrap().width, 1920);
        // Title bar above the screen.
        assert_eq!(WindowState { y: -500, ..w }.placed_on(&m), None);
        // Nonsense sizes.
        assert_eq!(WindowState { width: 10, ..w }.placed_on(&m), None);
    }

    #[test]
    fn store_names_only_from_safe_ids() {
        assert_eq!(
            term_store_name("ws_ab12", "m_cd34").as_deref(),
            Some("term-ws_ab12-m_cd34")
        );
        assert!(term_store_name("../x", "m").is_none());
        assert!(term_store_name("ws", "a/b").is_none());
        assert!(term_store_name("", "m").is_none());
    }
}
