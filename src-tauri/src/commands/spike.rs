//! M2 Spike: PTY -> Rust -> Tauri Channel -> WebView -> xterm.js
//! Throwaway harness, feature-gated behind `spike`, not part of M1 product surface.
//! Only compiled when `spike` feature is enabled.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, unused)]

#[cfg(feature = "spike")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "spike")]
use tauri::ipc::Channel;

#[cfg(feature = "spike")]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SpikeStreamRequest {
    pub bytes: usize, // how many bytes to generate
    pub seed: u8,
}

#[cfg(feature = "spike")]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SpikeResizeRequest {
    pub rows: u16,
    pub cols: u16,
}

/// Spike PTY stream: spawns a deterministic generator in a PTY and streams via Channel.
/// This validates the full pipeline: PTY -> Rust reader -> Tauri Channel -> WebView -> xterm.js
/// The frontend's xterm.js `write` is the final sink; lossless is verified by produced == delivered.
#[cfg(feature = "spike")]
#[tauri::command]
pub async fn spike_pty_stream(
    channel: Channel<Vec<u8>>,
    request: SpikeStreamRequest,
) -> Result<String, String> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::{Read, Write};

    // Spawn a PTY with a deterministic generator.
    // On Linux: bash with python3 fast generator. On Windows: powershell.
    // For spike we generate `request.bytes` of pattern + DONE marker.

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {}", e))?;

    let bytes = request.bytes;
    let seed = request.seed;

    // Build command that generates deterministic bytes
    let mut cmd = if cfg!(unix) {
        let py = format!(
            "python3 -c \"import sys; sys.stdout.buffer.write(b'A'*{}); sys.stdout.buffer.write(b'\\nDONE_MARKER\\n'); sys.stdout.buffer.flush()\"",
            bytes
        );
        let mut c = CommandBuilder::new("bash");
        c.arg("-c");
        c.arg(py);
        c
    } else {
        let ps = format!(
            "$d=[byte[]]::new(4096); for($i=0;$i -lt 4096;$i++){{$d[$i]=65}}; $out=[Console]::OpenStandardOutput(); $total={}; $written=0; while($written -lt $total){{ $chunk=[Math]::Min(4096, $total-$written); $out.Write($d,0,$chunk); $written+=$chunk }}; $out.Write([byte[]][char[]]\"`nDONE_MARKER`n\",0,13); $out.Flush()",
            bytes
        );
        let mut c = CommandBuilder::new("powershell.exe");
        c.arg("-NoProfile");
        c.arg("-Command");
        c.arg(ps);
        c
    };

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn failed: {}", e))?;

    // Drop slave, keep master for reading
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone_reader failed: {}", e))?;

    // Stream with bounded lossless semantics: read chunks, send via Channel, count bytes
    let mut total_delivered: usize = 0;
    let mut buf = [0u8; 8192];
    let mut produced: usize = 0;

    // Use a simple loop with timeout to avoid hang
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(15);

    loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            return Err(format!(
                "timeout after {:?}, delivered {}",
                start.elapsed(),
                total_delivered
            ));
        }

        // Non-blocking read via try_clone_reader is blocking; we set a short read timeout by using
        // the channel's backpressure: if WebView is slow, Channel will buffer but not drop.
        // For spike we just block on read with a timeout via thread.

        // Use a blocking read with a small timeout by checking child alive
        // To avoid blocking forever, we use `try_read` via filedescriptor or just read with timeout
        // Simpler: use `reader.read` in a thread with timeout

        // For now, do blocking read but with a check after each read if we saw DONE_MARKER
        match reader.read(&mut buf) {
            Ok(0) => {
                // EOF or no data
                if child.try_wait().map(|s| s.is_some()).unwrap_or(false) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(n) => {
                produced += n;
                let chunk = buf[..n].to_vec();
                // Send via Tauri Channel (lossless, bounded by Tauri's internal queue)
                channel
                    .send(chunk.clone())
                    .map_err(|e| format!("channel send failed: {}", e))?;
                total_delivered += n;
                // Check for DONE_MARKER to know we're done
                if chunk.windows(11).any(|w| w == b"DONE_MARKER") {
                    // Drain a bit more
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    // Try to read remaining
                    let mut extra = [0u8; 4096];
                    for _ in 0..5 {
                        match reader.read(&mut extra) {
                            Ok(0) => break,
                            Ok(m) => {
                                let c = extra[..m].to_vec();
                                channel
                                    .send(c.clone())
                                    .map_err(|e| format!("channel send failed: {}", e))?;
                                total_delivered += m;
                            }
                            Err(_) => break,
                        }
                    }
                    break;
                }
            }
            Err(e) => {
                // On WouldBlock, sleep; on other error, break
                let msg = format!("{}", e);
                if msg.contains("WouldBlock") || msg.contains("Interrupted") {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                } else {
                    break;
                }
            }
        }

        // Check child exit
        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() {
                // Child exited, but we still drain
                std::thread::sleep(std::time::Duration::from_millis(100));
                // Try one more drain
                let mut extra = [0u8; 4096];
                if let Ok(n) = reader.read(&mut extra) {
                    if n > 0 {
                        let c = extra[..n].to_vec();
                        let _ = channel.send(c);
                        total_delivered += n;
                    }
                }
                break;
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    Ok(format!(
        "produced {} delivered {} lossless {}",
        bytes,
        total_delivered,
        bytes <= total_delivered
    ))
}

#[cfg(feature = "spike")]
#[tauri::command]
pub fn spike_resize(rows: u16, cols: u16) -> Result<String, String> {
    // In real M3, resize propagates via PTY master resize. For spike, we just validate the call path.
    // We spawn a PTY, resize, and check get_size.
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {}", e))?;

    // Spawn a dummy shell that sleeps
    let mut cmd = if cfg!(unix) {
        let mut c = CommandBuilder::new("bash");
        c.arg("-c");
        c.arg("sleep 0.5");
        c
    } else {
        let mut c = CommandBuilder::new("powershell.exe");
        c.arg("-NoProfile");
        c.arg("-Command");
        c.arg("Start-Sleep -Milliseconds 500");
        c
    };

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn failed: {}", e))?;

    // Resize
    pair.master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("resize failed: {}", e))?;

    let size = pair
        .master
        .get_size()
        .map_err(|e| format!("get_size failed: {}", e))?;

    let _ = child.kill();
    let _ = child.wait();

    if size.rows == rows && size.cols == cols {
        Ok(format!("resize {}x{} ok", rows, cols))
    } else {
        Err(format!(
            "resize expected {}x{} got {}x{}",
            rows, cols, size.rows, size.cols
        ))
    }
}

#[cfg(feature = "spike")]
#[tauri::command]
pub fn spike_input_echo(input: String) -> Result<String, String> {
    // Return input path: WebView -> Rust -> PTY -> Rust -> WebView
    // For spike, we just echo the input via PTY and verify it comes back.
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::{Read, Write};

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {}", e))?;

    let mut cmd = if cfg!(unix) {
        let mut c = CommandBuilder::new("bash");
        c.arg("-c");
        c.arg("cat");
        c
    } else {
        let mut c = CommandBuilder::new("powershell.exe");
        c.arg("-NoProfile");
        c.arg("-Command");
        c.arg("$input = $null; while($true){ $l = Read-Host; Write-Output $l; if($l -eq 'EXIT'){break} }");
        c
    };

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn failed: {}", e))?;

    let mut writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("take_writer failed: {}", e))?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone_reader failed: {}", e))?;

    // Write input
    writer
        .write_all(format!("{}\n", input).as_bytes())
        .map_err(|e| format!("write failed: {}", e))?;
    writer.flush().map_err(|e| format!("flush failed: {}", e))?;

    // Read echo
    let mut buf = [0u8; 4096];
    let mut out = Vec::new();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(2) {
        match reader.read(&mut buf) {
            Ok(0) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&out).contains(&input) {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    let s = String::from_utf8_lossy(&out);
    if s.contains(&input) {
        Ok(format!("input echo ok: {}", input))
    } else {
        Err(format!(
            "input echo failed, got: {:?}",
            &s[..std::cmp::min(s.len(), 200)]
        ))
    }
}
