//! Bounded reader for Windows Shell Link (`.lnk`) files per [MS-SHLLINK].
//!
//! Stored-metadata policy (M6 decision, PLATFORM_ADAPTERS §3 option (a)):
//! ToolOnize reads the link's stored fields and never calls
//! `IShellLink::Resolve`, so discovery cannot show UI, search, silently
//! re-target, or mutate a link (T-WIN-01). Being pure Rust, the reader is
//! exercised on every platform with generated fixtures.

pub const MAX_LNK_BYTES: usize = 1024 * 1024;
const HEADER_SIZE: u32 = 0x4C;
const CLSID: [u8; 16] = [
    0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

const HAS_ID_LIST: u32 = 0x1;
const HAS_LINK_INFO: u32 = 0x2;
const HAS_NAME: u32 = 0x4;
const HAS_RELATIVE_PATH: u32 = 0x8;
const HAS_WORKING_DIR: u32 = 0x10;
const HAS_ARGUMENTS: u32 = 0x20;
const HAS_ICON_LOCATION: u32 = 0x40;
const IS_UNICODE: u32 = 0x80;

const ENV_BLOCK: u32 = 0xA000_0001;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShellLink {
    pub local_base_path: Option<String>,
    pub network_path: Option<String>,
    pub env_target: Option<String>,
    pub has_id_list: bool,
    pub name: Option<String>,
    pub relative_path: Option<String>,
    pub working_dir: Option<String>,
    pub arguments: Option<String>,
    pub icon_location: Option<String>,
    pub show_command: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LnkError {
    TooLarge,
    Truncated(&'static str),
    BadHeader,
    BadStructure(&'static str),
}

impl std::fmt::Display for LnkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LnkError::TooLarge => write!(f, "shortcut exceeds {MAX_LNK_BYTES} bytes"),
            LnkError::Truncated(what) => write!(f, "shortcut truncated in {what}"),
            LnkError::BadHeader => f.write_str("not a Shell Link file"),
            LnkError::BadStructure(what) => write!(f, "malformed shortcut: {what}"),
        }
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize, what: &'static str) -> Result<&'a [u8], LnkError> {
        let end = self.pos.checked_add(n).ok_or(LnkError::Truncated(what))?;
        let slice = self
            .data
            .get(self.pos..end)
            .ok_or(LnkError::Truncated(what))?;
        self.pos = end;
        Ok(slice)
    }
    fn u16(&mut self, what: &'static str) -> Result<u16, LnkError> {
        let b = self.take(2, what)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self, what: &'static str) -> Result<u32, LnkError> {
        let b = self.take(4, what)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

fn le_u32(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// NUL-terminated single-byte string (system code page; non-ASCII bytes are
/// decoded as Latin-1, which is lossless for display/review purposes).
fn c_string(data: &[u8], at: usize) -> Option<String> {
    let rest = data.get(at..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    Some(rest[..end].iter().map(|&b| b as char).collect())
}

fn c_wstring(data: &[u8], at: usize) -> Option<String> {
    let rest = data.get(at..)?;
    let units: Vec<u16> = rest
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    if units.len() * 2 >= rest.len() {
        return None; // no terminator inside the block
    }
    Some(String::from_utf16_lossy(&units))
}

fn parse_link_info(block: &[u8], link: &mut ShellLink) -> Result<(), LnkError> {
    let header_size = le_u32(block, 4).ok_or(LnkError::Truncated("LinkInfo"))? as usize;
    let flags = le_u32(block, 8).ok_or(LnkError::Truncated("LinkInfo"))?;
    let local_off = le_u32(block, 16).ok_or(LnkError::Truncated("LinkInfo"))? as usize;
    let net_off = le_u32(block, 20).ok_or(LnkError::Truncated("LinkInfo"))? as usize;
    let suffix_off = le_u32(block, 24).ok_or(LnkError::Truncated("LinkInfo"))? as usize;
    let (local_uni, suffix_uni) = if header_size >= 0x24 {
        (
            le_u32(block, 28).map(|v| v as usize),
            le_u32(block, 32).map(|v| v as usize),
        )
    } else {
        (None, None)
    };
    let suffix = suffix_uni
        .filter(|&o| o > 0)
        .and_then(|o| c_wstring(block, o))
        .or_else(|| {
            (suffix_off > 0)
                .then(|| c_string(block, suffix_off))
                .flatten()
        })
        .unwrap_or_default();
    if flags & 0x1 != 0 {
        let base = local_uni
            .filter(|&o| o > 0)
            .and_then(|o| c_wstring(block, o))
            .or_else(|| c_string(block, local_off))
            .ok_or(LnkError::BadStructure("LocalBasePath"))?;
        link.local_base_path = Some(format!("{base}{suffix}"));
    }
    if flags & 0x2 != 0 {
        // CommonNetworkRelativeLink: NetName offset at +8 within the block.
        let net_name = le_u32(block, net_off + 8)
            .and_then(|o| c_string(block, net_off + o as usize))
            .unwrap_or_else(|| "\\\\<network>".into());
        let joined = if suffix.is_empty() {
            net_name
        } else {
            format!("{net_name}\\{suffix}")
        };
        link.network_path = Some(joined);
    }
    Ok(())
}

pub fn parse(data: &[u8]) -> Result<ShellLink, LnkError> {
    if data.len() > MAX_LNK_BYTES {
        return Err(LnkError::TooLarge);
    }
    let mut c = Cursor { data, pos: 0 };
    if c.u32("header")? != HEADER_SIZE || c.take(16, "header")? != CLSID {
        return Err(LnkError::BadHeader);
    }
    let flags = c.u32("header")?;
    let _attributes = c.u32("header")?;
    c.take(24, "header")?; // creation/access/write times
    let _file_size = c.u32("header")?;
    let _icon_index = c.u32("header")?;
    let show_command = c.u32("header")?;
    c.take(12, "header")?; // hotkey + reserved

    let mut link = ShellLink {
        has_id_list: flags & HAS_ID_LIST != 0,
        show_command,
        ..Default::default()
    };
    if flags & HAS_ID_LIST != 0 {
        let size = c.u16("IDList")? as usize;
        c.take(size, "IDList")?;
    }
    if flags & HAS_LINK_INFO != 0 {
        let start = c.pos;
        let size = c.u32("LinkInfo")? as usize;
        if size < 0x1C {
            return Err(LnkError::BadStructure("LinkInfo size"));
        }
        c.pos = start;
        let block = c.take(size, "LinkInfo")?;
        parse_link_info(block, &mut link)?;
    }
    let unicode = flags & IS_UNICODE != 0;
    let string = |c: &mut Cursor<'_>| -> Result<String, LnkError> {
        let count = c.u16("StringData")? as usize;
        if unicode {
            let bytes = c.take(count * 2, "StringData")?;
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect();
            Ok(String::from_utf16_lossy(&units))
        } else {
            Ok(c.take(count, "StringData")?
                .iter()
                .map(|&b| b as char)
                .collect())
        }
    };
    if flags & HAS_NAME != 0 {
        link.name = Some(string(&mut c)?);
    }
    if flags & HAS_RELATIVE_PATH != 0 {
        link.relative_path = Some(string(&mut c)?);
    }
    if flags & HAS_WORKING_DIR != 0 {
        link.working_dir = Some(string(&mut c)?);
    }
    if flags & HAS_ARGUMENTS != 0 {
        link.arguments = Some(string(&mut c)?);
    }
    if flags & HAS_ICON_LOCATION != 0 {
        link.icon_location = Some(string(&mut c)?);
    }
    // ExtraData: only the EnvironmentVariableDataBlock matters for targets.
    while let Ok(size) = c.u32("ExtraData") {
        if size < 4 {
            break;
        }
        let size = size as usize;
        let block = c.take(size - 4, "ExtraData")?;
        if size >= 8 && le_u32(block, 0) == Some(ENV_BLOCK) && block.len() >= 4 + 260 + 520 {
            let unicode_target = c_wstring(&block[4 + 260..4 + 260 + 520], 0);
            let ansi_target = c_string(&block[4..4 + 260], 0);
            link.env_target = unicode_target
                .filter(|s| !s.is_empty())
                .or(ansi_target.filter(|s| !s.is_empty()));
        }
    }
    Ok(link)
}

/// Expand `%VAR%` references using `lookup`. Unknown variables are kept and
/// reported (the record then needs review).
pub fn expand_env(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut unknown = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 && !after[..end].contains(['\\', '/', ' ']) => {
                let name = &after[..end];
                match lookup(name) {
                    Some(value) => out.push_str(&value),
                    None => {
                        unknown.push(name.to_string());
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    (out, unknown)
}

/// Test-only writer used to generate synthetic fixtures at test time
/// (never committed binaries, TEST_STRATEGY §6).
#[cfg(test)]
pub mod build {
    use super::*;

    #[derive(Default)]
    pub struct LnkSpec<'a> {
        pub local_base_path: Option<&'a str>,
        pub network: Option<(&'a str, &'a str)>,
        pub env_target: Option<&'a str>,
        pub id_list: bool,
        pub name: Option<&'a str>,
        pub relative_path: Option<&'a str>,
        pub working_dir: Option<&'a str>,
        pub arguments: Option<&'a str>,
        pub icon_location: Option<&'a str>,
    }

    fn wstr(out: &mut Vec<u8>, s: &str) {
        let units: Vec<u16> = s.encode_utf16().collect();
        out.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for u in units {
            out.extend_from_slice(&u.to_le_bytes());
        }
    }

    pub fn build(spec: &LnkSpec<'_>) -> Vec<u8> {
        let mut flags = IS_UNICODE;
        if spec.id_list {
            flags |= HAS_ID_LIST;
        }
        if spec.local_base_path.is_some() || spec.network.is_some() {
            flags |= HAS_LINK_INFO;
        }
        for (present, bit) in [
            (spec.name.is_some(), HAS_NAME),
            (spec.relative_path.is_some(), HAS_RELATIVE_PATH),
            (spec.working_dir.is_some(), HAS_WORKING_DIR),
            (spec.arguments.is_some(), HAS_ARGUMENTS),
            (spec.icon_location.is_some(), HAS_ICON_LOCATION),
        ] {
            if present {
                flags |= bit;
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&HEADER_SIZE.to_le_bytes());
        out.extend_from_slice(&CLSID);
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&[0u8; 24]);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&[0u8; 12]);
        if spec.id_list {
            out.extend_from_slice(&4u16.to_le_bytes());
            out.extend_from_slice(&[2, 0, 0, 0]);
        }
        if spec.local_base_path.is_some() || spec.network.is_some() {
            let mut info = vec![0u8; 0x1C];
            let mut li_flags = 0u32;
            let mut local_off = 0u32;
            let mut net_off = 0u32;
            if let Some(path) = spec.local_base_path {
                li_flags |= 1;
                local_off = info.len() as u32;
                info.extend_from_slice(path.as_bytes());
                info.push(0);
            }
            let suffix_off;
            if let Some((share, suffix)) = spec.network {
                li_flags |= 2;
                net_off = info.len() as u32;
                let mut net = vec![0u8; 0x14];
                net[..4].copy_from_slice(&0u32.to_le_bytes());
                net[8..12].copy_from_slice(&0x14u32.to_le_bytes());
                net.extend_from_slice(share.as_bytes());
                net.push(0);
                let size = net.len() as u32;
                net[..4].copy_from_slice(&size.to_le_bytes());
                info.extend_from_slice(&net);
                suffix_off = info.len() as u32;
                info.extend_from_slice(suffix.as_bytes());
                info.push(0);
            } else {
                suffix_off = info.len() as u32;
                info.push(0);
            }
            let size = info.len() as u32;
            info[0..4].copy_from_slice(&size.to_le_bytes());
            info[4..8].copy_from_slice(&0x1Cu32.to_le_bytes());
            info[8..12].copy_from_slice(&li_flags.to_le_bytes());
            info[16..20].copy_from_slice(&local_off.to_le_bytes());
            info[20..24].copy_from_slice(&net_off.to_le_bytes());
            info[24..28].copy_from_slice(&suffix_off.to_le_bytes());
            out.extend_from_slice(&info);
        }
        for s in [
            spec.name,
            spec.relative_path,
            spec.working_dir,
            spec.arguments,
            spec.icon_location,
        ]
        .into_iter()
        .flatten()
        {
            wstr(&mut out, s);
        }
        if let Some(target) = spec.env_target {
            let mut block = Vec::new();
            block.extend_from_slice(&0x314u32.to_le_bytes());
            block.extend_from_slice(&ENV_BLOCK.to_le_bytes());
            let mut ansi = [0u8; 260];
            ansi[..target.len().min(259)]
                .copy_from_slice(&target.as_bytes()[..target.len().min(259)]);
            block.extend_from_slice(&ansi);
            let mut wide = [0u8; 520];
            for (i, u) in target.encode_utf16().take(259).enumerate() {
                wide[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            block.extend_from_slice(&wide);
            out.extend_from_slice(&block);
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::build::{build, LnkSpec};
    use super::*;

    #[test]
    fn reads_stored_fields() {
        let bytes = build(&LnkSpec {
            local_base_path: Some("C:\\Program Files\\Example\\example.exe"),
            name: Some("Example Tool"),
            working_dir: Some("C:\\Users\\Example"),
            arguments: Some("--profile \"work dir\" & whoami"),
            icon_location: Some("C:\\Program Files\\Example\\example.ico"),
            id_list: true,
            ..Default::default()
        });
        let link = parse(&bytes).unwrap();
        assert_eq!(
            link.local_base_path.as_deref(),
            Some("C:\\Program Files\\Example\\example.exe")
        );
        assert_eq!(link.name.as_deref(), Some("Example Tool"));
        assert_eq!(link.working_dir.as_deref(), Some("C:\\Users\\Example"));
        assert_eq!(
            link.arguments.as_deref(),
            Some("--profile \"work dir\" & whoami"),
            "argument string is kept verbatim"
        );
        assert!(link.has_id_list);
    }

    #[test]
    fn network_and_env_targets() {
        let link = parse(&build(&LnkSpec {
            network: Some(("\\\\fileserver.example.com\\tools", "example.exe")),
            ..Default::default()
        }))
        .unwrap();
        assert_eq!(
            link.network_path.as_deref(),
            Some("\\\\fileserver.example.com\\tools\\example.exe")
        );
        let link = parse(&build(&LnkSpec {
            env_target: Some("%ProgramFiles%\\Example\\example.exe"),
            ..Default::default()
        }))
        .unwrap();
        assert_eq!(
            link.env_target.as_deref(),
            Some("%ProgramFiles%\\Example\\example.exe")
        );
    }

    #[test]
    fn rejects_garbage_and_truncation_t_disc_win_007() {
        assert_eq!(parse(b"not a link"), Err(LnkError::BadHeader));
        assert_eq!(parse(b"ab"), Err(LnkError::Truncated("header")));
        let mut bytes = build(&LnkSpec {
            local_base_path: Some("C:\\x.exe"),
            arguments: Some("abc"),
            ..Default::default()
        });
        bytes[4] ^= 0xff;
        assert_eq!(parse(&bytes), Err(LnkError::BadHeader));
        let good = build(&LnkSpec {
            local_base_path: Some("C:\\x.exe"),
            arguments: Some("abcdef"),
            ..Default::default()
        });
        for cut in 0..good.len() - 4 {
            // Every truncation point fails cleanly (or parses a prefix), never panics.
            let _ = parse(&good[..cut]);
        }
        assert_eq!(
            parse(&vec![0u8; MAX_LNK_BYTES + 1]),
            Err(LnkError::TooLarge)
        );
    }

    #[test]
    fn env_expansion() {
        let lookup = |name: &str| match name {
            "ProgramFiles" => Some("C:\\Program Files".to_string()),
            _ => None,
        };
        assert_eq!(
            expand_env("%ProgramFiles%\\Example\\x.exe", &lookup),
            ("C:\\Program Files\\Example\\x.exe".into(), vec![])
        );
        let (out, unknown) = expand_env("%NOPE%\\x.exe 100%", &lookup);
        assert_eq!(out, "%NOPE%\\x.exe 100%");
        assert_eq!(unknown, vec!["NOPE"]);
    }

    proptest::proptest! {
        #[test]
        fn never_panics(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512)) {
            let mut data = Vec::new();
            data.extend_from_slice(&HEADER_SIZE.to_le_bytes());
            data.extend_from_slice(&CLSID);
            data.extend_from_slice(&bytes);
            let _ = parse(&data);
        }
    }
}
