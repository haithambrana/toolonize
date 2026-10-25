//! Bounded Desktop Entry keyfile parser (Desktop Entry Specification §3-§4).
//!
//! Hostile-input posture (T-DISC-02): size/count limits, strict UTF-8,
//! duplicate groups/keys rejected, never panics. Unknown keys and groups are
//! tolerated.

pub const MAX_FILE_BYTES: usize = 64 * 1024;
const MAX_GROUPS: usize = 64;
const MAX_KEYS: usize = 1024;
const MAX_LINE: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub locale: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyFile {
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyFileError {
    TooLarge,
    InvalidUtf8,
    TooManyGroups,
    TooManyKeys,
    LineTooLong(usize),
    EntryOutsideGroup(usize),
    MalformedGroup(usize),
    MalformedLine(usize),
    DuplicateGroup(String),
    DuplicateKey(String),
}

impl std::fmt::Display for KeyFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "file exceeds {MAX_FILE_BYTES} bytes"),
            Self::InvalidUtf8 => f.write_str("file is not valid UTF-8"),
            Self::TooManyGroups => f.write_str("too many groups"),
            Self::TooManyKeys => f.write_str("too many keys"),
            Self::LineTooLong(l) => write!(f, "line {l} is too long"),
            Self::EntryOutsideGroup(l) => write!(f, "line {l}: key before first group"),
            Self::MalformedGroup(l) => write!(f, "line {l}: malformed group header"),
            Self::MalformedLine(l) => write!(f, "line {l}: malformed entry"),
            Self::DuplicateGroup(g) => write!(f, "duplicate group [{g}]"),
            Self::DuplicateKey(k) => write!(f, "duplicate key {k}"),
        }
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn has_control(s: &str) -> bool {
    s.chars().any(|c| c.is_control() && c != '\t')
}

impl KeyFile {
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, KeyFileError> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(KeyFileError::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| KeyFileError::InvalidUtf8)?;
        Self::parse(text)
    }

    pub fn parse(text: &str) -> Result<Self, KeyFileError> {
        if text.len() > MAX_FILE_BYTES {
            return Err(KeyFileError::TooLarge);
        }
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut file = KeyFile::default();
        let mut keys = 0usize;
        for (index, raw) in text.split('\n').enumerate() {
            let line_no = index + 1;
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.len() > MAX_LINE {
                return Err(KeyFileError::LineTooLong(line_no));
            }
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix('[') {
                let name = rest
                    .trim_end()
                    .strip_suffix(']')
                    .ok_or(KeyFileError::MalformedGroup(line_no))?;
                if name.is_empty() || name.contains(['[', ']']) || has_control(name) {
                    return Err(KeyFileError::MalformedGroup(line_no));
                }
                if file.groups.iter().any(|g| g.name == name) {
                    return Err(KeyFileError::DuplicateGroup(name.to_string()));
                }
                if file.groups.len() >= MAX_GROUPS {
                    return Err(KeyFileError::TooManyGroups);
                }
                file.groups.push(Group {
                    name: name.to_string(),
                    entries: Vec::new(),
                });
                continue;
            }
            let group = file
                .groups
                .last_mut()
                .ok_or(KeyFileError::EntryOutsideGroup(line_no))?;
            let (lhs, value) = line
                .split_once('=')
                .ok_or(KeyFileError::MalformedLine(line_no))?;
            let lhs = lhs.trim();
            let value = value.trim_start();
            let (key, locale) = match lhs.split_once('[') {
                Some((key, rest)) => {
                    let locale = rest
                        .strip_suffix(']')
                        .ok_or(KeyFileError::MalformedLine(line_no))?;
                    if locale.is_empty() || locale.contains(['[', ']', '=']) {
                        return Err(KeyFileError::MalformedLine(line_no));
                    }
                    (key, Some(locale.to_string()))
                }
                None => (lhs, None),
            };
            if !valid_key(key) || has_control(value) {
                return Err(KeyFileError::MalformedLine(line_no));
            }
            if group
                .entries
                .iter()
                .any(|e| e.key == key && e.locale == locale)
            {
                return Err(KeyFileError::DuplicateKey(key.to_string()));
            }
            keys += 1;
            if keys > MAX_KEYS {
                return Err(KeyFileError::TooManyKeys);
            }
            group.entries.push(Entry {
                key: key.to_string(),
                locale,
                value: value.to_string(),
            });
        }
        Ok(file)
    }

    pub fn group(&self, name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name == name)
    }
}

impl Group {
    /// Raw (still escaped) value of an unlocalized key.
    pub fn raw(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.key == key && e.locale.is_none())
            .map(|e| e.value.as_str())
    }

    /// Unescaped string value (`\s \n \t \r \\`).
    pub fn string(&self, key: &str) -> Option<String> {
        self.raw(key).map(unescape)
    }

    /// Best localized value for `locales` (most specific first), falling
    /// back to the unlocalized key.
    pub fn localized(&self, key: &str, locales: &[String]) -> Option<String> {
        for locale in locales {
            if let Some(entry) = self
                .entries
                .iter()
                .find(|e| e.key == key && e.locale.as_deref() == Some(locale.as_str()))
            {
                return Some(unescape(&entry.value));
            }
        }
        self.string(key)
    }

    pub fn boolean(&self, key: &str) -> Option<bool> {
        match self.raw(key)?.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    /// `;`-separated list, honouring `\;` escapes.
    pub fn list(&self, key: &str) -> Vec<String> {
        let Some(raw) = self.raw(key) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let mut current = String::new();
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some(';') => current.push(';'),
                    Some(other) => {
                        current.push('\\');
                        current.push(other);
                    }
                    None => current.push('\\'),
                },
                ';' => items.push(unescape(&std::mem::take(&mut current))),
                other => current.push(other),
            }
        }
        if !current.is_empty() {
            items.push(unescape(&current));
        }
        items.retain(|i| !i.is_empty());
        items
    }
}

/// Desktop Entry string escapes. Unknown escapes are kept verbatim.
pub fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Locale preference list derived from LC_ALL / LC_MESSAGES / LANG values,
/// following the spec's lang_COUNTRY@MODIFIER matching order.
pub fn locale_candidates(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.filter(|r| !r.is_empty() && *r != "C" && *r != "POSIX") else {
        return Vec::new();
    };
    let without_encoding = match raw.split_once('.') {
        Some((head, tail)) => match tail.split_once('@') {
            Some((_, modifier)) => format!("{head}@{modifier}"),
            None => head.to_string(),
        },
        None => raw.to_string(),
    };
    let (base, modifier) = match without_encoding.split_once('@') {
        Some((b, m)) => (b.to_string(), Some(m.to_string())),
        None => (without_encoding.clone(), None),
    };
    let (lang, country) = match base.split_once('_') {
        Some((l, c)) => (l.to_string(), Some(c.to_string())),
        None => (base.clone(), None),
    };
    let mut out = Vec::new();
    if let (Some(c), Some(m)) = (&country, &modifier) {
        out.push(format!("{lang}_{c}@{m}"));
    }
    if let Some(c) = &country {
        out.push(format!("{lang}_{c}"));
    }
    if let Some(m) = &modifier {
        out.push(format!("{lang}@{m}"));
    }
    out.push(lang);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "[Desktop Entry]\nType=Application\nName=Example Tool\nName[de]=Beispiel\nExec=example-tool %U\nCategories=Development;Utility;\n";

    #[test]
    fn parses_minimal_entry() {
        let file = KeyFile::parse(MINIMAL).unwrap();
        let g = file.group("Desktop Entry").unwrap();
        assert_eq!(g.string("Name").as_deref(), Some("Example Tool"));
        assert_eq!(
            g.localized("Name", &["de_DE".into(), "de".into()])
                .as_deref(),
            Some("Beispiel")
        );
        assert_eq!(g.list("Categories"), vec!["Development", "Utility"]);
    }

    #[test]
    fn tolerates_unknown_keys_groups_crlf_bom_and_comments() {
        let text = "\u{feff}# comment\r\n[Desktop Entry]\r\nType=Application\r\nX-Custom-Key=1\r\n\r\n[Desktop Action new]\r\nName=New\r\n";
        let file = KeyFile::parse(text).unwrap();
        assert_eq!(file.groups.len(), 2);
        assert_eq!(
            file.group("Desktop Entry").unwrap().raw("X-Custom-Key"),
            Some("1")
        );
    }

    #[test]
    fn rejects_hostile_shapes() {
        assert_eq!(
            KeyFile::parse("[A]\n[A]\n"),
            Err(KeyFileError::DuplicateGroup("A".into()))
        );
        assert_eq!(
            KeyFile::parse("[A]\nk=1\nk=2\n"),
            Err(KeyFileError::DuplicateKey("k".into()))
        );
        assert_eq!(
            KeyFile::parse("k=1\n[A]\n"),
            Err(KeyFileError::EntryOutsideGroup(1))
        );
        assert!(KeyFile::parse("[A\n").is_err());
        assert!(KeyFile::parse("[A]\nno equals sign\n").is_err());
        assert!(KeyFile::parse("[A]\nbad key=1\n").is_err());
        assert!(KeyFile::parse("[A]\nk=a\u{7}b\n").is_err());
        assert_eq!(
            KeyFile::parse_bytes(&[b'[', 0xff, b']']),
            Err(KeyFileError::InvalidUtf8)
        );
        let huge = "x".repeat(MAX_FILE_BYTES + 1);
        assert_eq!(KeyFile::parse(&huge), Err(KeyFileError::TooLarge));
        let long_line = format!("[A]\nk={}\n", "y".repeat(MAX_LINE + 1));
        assert!(matches!(
            KeyFile::parse(&long_line),
            Err(KeyFileError::LineTooLong(2))
        ));
    }

    #[test]
    fn unescapes_string_values() {
        assert_eq!(unescape(r"a\sb\\c\td\x"), "a b\\c\td\\x");
    }

    #[test]
    fn list_handles_escaped_separator() {
        let file = KeyFile::parse("[A]\nK=one\\;two;three\n").unwrap();
        assert_eq!(file.group("A").unwrap().list("K"), vec!["one;two", "three"]);
    }

    #[test]
    fn locale_candidate_order() {
        assert_eq!(
            locale_candidates(Some("sr_YU.UTF-8@Latn")),
            vec!["sr_YU@Latn", "sr_YU", "sr@Latn", "sr"]
        );
        assert_eq!(locale_candidates(Some("de_DE.UTF-8")), vec!["de_DE", "de"]);
        assert!(locale_candidates(Some("C")).is_empty());
        assert!(locale_candidates(None).is_empty());
    }

    proptest::proptest! {
        #[test]
        fn never_panics_on_arbitrary_input(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..2048)) {
            let _ = KeyFile::parse_bytes(&bytes);
        }
    }
}
