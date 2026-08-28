//! Deterministic synthetic child fixtures (no network, no secrets, no personal data).
//! All fixtures are reproducible and produce known byte patterns for lossless verification.

/// Generate deterministic pattern of given length: repeating 0..255 or ASCII.
/// Returns Vec<u8> and its checksum (simple sum).
pub fn generate_pattern(len: usize, seed: u8) -> (Vec<u8>, u64) {
    let mut v = Vec::with_capacity(len);
    let mut sum: u64 = 0;
    for i in 0..len {
        let b = ((i as u8).wrapping_add(seed)) % 94 + 33; // printable 33..126
        if i % 1024 == 1023 {
            v.push(b'\n');
            sum = sum.wrapping_add(b'\n' as u64);
        } else {
            v.push(b);
            sum = sum.wrapping_add(b as u64);
        }
    }
    (v, sum)
}

/// UTF-8 fixture strings covering emoji, CJK, accented, RTL, combining.
pub fn utf8_fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        ("emoji", "Hello 🌍 🌟 🚀 👩‍💻 — test 🎉"),
        ("cjk", "こんにちは世界 你好世界 안녕하세요"),
        ("accented", "café naïve résumé façade coöperate"),
        ("combining", "e\u{0301} a\u{0308} o\u{0302}"), // e + combining acute etc.
        ("mixed", "Mix: ASCII 123, emoji 🦀, CJK 中文, accented café"),
    ]
}

/// Shell commands for synthetic fixtures per platform.
/// These are invoked via PTY's shell, not via direct exec of fixture binaries,
/// so we use standard shells with deterministic output.
pub fn shell_for_platform() -> (&'static str, Vec<String>) {
    #[cfg(windows)]
    {
        // Try PowerShell first, fallback to cmd
        (
            "powershell.exe",
            vec!["-NoProfile".to_string(), "-Command".to_string()],
        )
    }
    #[cfg(unix)]
    {
        ("bash", vec!["-c".to_string()])
    }
}

/// Generate a high-volume output command string.
/// On Unix: `yes` or `python3 -c` or `seq`; we use a deterministic bash loop.
/// On Windows: PowerShell loop.
pub fn high_volume_cmd(bytes: usize) -> (String, Vec<String>) {
    #[cfg(unix)]
    {
        // Use bash with yes and head to generate deterministic bytes
        // `yes` generates "y\n" repeatedly; we use `head -c` to limit.
        // For lossless test we generate pattern via python if available, else yes.
        let cmd = "bash".to_string();
        let script = format!("python3 -c \"import sys; data = b'TOOLONIZE_PATTERN_0123456789_abcdefghijklmnopqrstuvwxyz_'*1024; total={}; w=sys.stdout.buffer.write; [w(data) for _ in range((total//len(data))+1)]; w(data[:total%len(data)])\" | head -c {}", bytes, bytes);
        // Actually simpler: use `head -c` from /dev/zero with tr
        // Fallback to `yes` approach:
        // let script = format!("yes 'X' | tr -d '\\n' | head -c {} ; echo", bytes);
        (cmd, vec!["-c".to_string(), script])
    }
    #[cfg(windows)]
    {
        let cmd = "powershell.exe".to_string();
        let ps = format!("$s='TOOLONIZE_PATTERN_0123456789_abcdefghijklmnopqrstuvwxyz_'*1024; $b=[System.Text.Encoding]::ASCII.GetBytes($s); $out=[Console]::OpenStandardOutput(); $total={}; $written=0; while($written -lt $total){{ $toWrite=[Math]::Min($b.Length, $total-$written); $out.Write($b,0,$toWrite); $written+=$toWrite }}", bytes);
        (
            cmd,
            vec!["-NoProfile".to_string(), "-Command".to_string(), ps],
        )
    }
}

/// Invalid executable fixture - nonexistent path.
pub fn invalid_exe() -> (&'static str, Vec<&'static str>) {
    ("/nonexistent/invalid_executable_xyz_12345", vec![])
}

/// Resize test: echo stty size or mode con
pub fn resize_check_cmd() -> (String, Vec<String>) {
    #[cfg(unix)]
    {
        (
            "bash".to_string(),
            vec![
                "-c".to_string(),
                "stty size; echo RESIZE_CHECK_DONE".to_string(),
            ],
        )
    }
    #[cfg(windows)]
    {
        (
            "powershell.exe".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "mode con | Select-String -Pattern 'Columns|Lines'; echo RESIZE_CHECK_DONE"
                    .to_string(),
            ],
        )
    }
}
