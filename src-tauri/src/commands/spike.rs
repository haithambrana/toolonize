//! Throwaway M2 proof for PTY -> Rust -> Tauri Channel -> WebView -> xterm.js.

use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;

const MARKER: &[u8] = b"DONE_MARKER";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeStreamRequest {
    pub bytes: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeStreamResult {
    pub payload_bytes: usize,
    pub streamed_bytes: usize,
    pub marker_bytes: usize,
    pub process_exit_code: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeResizeResult {
    pub requested_rows: u16,
    pub requested_cols: u16,
    pub observed_rows: u16,
    pub observed_cols: u16,
    pub process_exit_code: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeInputResult {
    pub echoed: String,
    pub process_exit_code: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeBrowserReport {
    pub payload_bytes: usize,
    pub delivered_payload_bytes: usize,
    pub expected_sha256: String,
    pub delivered_sha256: String,
    pub exact_byte_integrity: bool,
    pub xterm_write_completed: bool,
    pub input_return: bool,
    pub real_resize: bool,
    pub process_exit_code: u32,
}

fn browser_report_is_valid(report: &SpikeBrowserReport) -> bool {
    report.payload_bytes == report.delivered_payload_bytes
        && report.expected_sha256.len() == 64
        && report.expected_sha256 == report.delivered_sha256
        && report.exact_byte_integrity
        && report.xterm_write_completed
        && report.input_return
        && report.real_resize
        && report.process_exit_code == 0
}

enum ReadEvent {
    Data(Vec<u8>),
    Eof,
    Error(String),
}

fn reader_events(mut reader: Box<dyn Read + Send>) -> Receiver<ReadEvent> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buffer = vec![0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let _ = sender.send(ReadEvent::Eof);
                    break;
                }
                Ok(count) => {
                    if sender
                        .send(ReadEvent::Data(buffer[..count].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(ReadEvent::Error(error.to_string()));
                    break;
                }
            }
        }
    });
    receiver
}

fn receive_until_eof(
    receiver: &Receiver<ReadEvent>,
    child: &mut dyn Child,
) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let mut output = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            return Err("PTY read timed out".to_string());
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(250))) {
            Ok(ReadEvent::Data(data)) => output.extend(data),
            Ok(ReadEvent::Eof) => return Ok(output),
            Ok(ReadEvent::Error(error)) => return Err(format!("PTY read failed: {error}")),
            Err(RecvTimeoutError::Timeout) => {
                if child
                    .try_wait()
                    .map_err(|error| error.to_string())?
                    .is_some()
                {
                    continue;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("PTY reader disconnected before EOF".to_string());
            }
        }
    }
}

fn generator_command(bytes: usize) -> CommandBuilder {
    if cfg!(unix) {
        let mut command = CommandBuilder::new("python3");
        command.arg("-c");
        command.arg(format!(
            "import sys; sys.stdout.buffer.write(b'A'*{bytes}+b'DONE_MARKER'); sys.stdout.buffer.flush()"
        ));
        command
    } else {
        let mut command = CommandBuilder::new("powershell.exe");
        command.arg("-NoProfile");
        command.arg("-Command");
        command.arg(format!(
            "$d=[byte[]]::new(4096); [Array]::Fill($d,[byte]65); $o=[Console]::OpenStandardOutput(); $left={bytes}; while($left -gt 0){{$n=[Math]::Min(4096,$left); $o.Write($d,0,$n); $left-=$n}}; $m=[Text.Encoding]::ASCII.GetBytes('DONE_MARKER'); $o.Write($m,0,$m.Length); $o.Flush()"
        ));
        command
    }
}

fn wait_success(child: &mut dyn Child) -> Result<u32, String> {
    let status = child.wait().map_err(|error| error.to_string())?;
    let code = status.exit_code();
    if status.success() {
        Ok(code)
    } else {
        Err(format!("PTY child exited with code {code}"))
    }
}

fn stream_pty(
    channel: Channel<Vec<u8>>,
    request: SpikeStreamRequest,
) -> Result<SpikeStreamResult, String> {
    if request.bytes == 0 || request.bytes > 2 * 1024 * 1024 {
        return Err("payload must be between 1 byte and 2 MiB".to_string());
    }
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("openpty failed: {error}"))?;
    let mut child = pair
        .slave
        .spawn_command(generator_command(request.bytes))
        .map_err(|error| format!("spawn failed: {error}"))?;
    let receiver = reader_events(
        pair.master
            .try_clone_reader()
            .map_err(|error| format!("clone reader failed: {error}"))?,
    );
    drop(pair.slave);

    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let mut streamed = 0;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            return Err(format!("PTY stream timed out after {streamed} bytes"));
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(250))) {
            Ok(ReadEvent::Data(data)) => {
                streamed += data.len();
                channel
                    .send(data)
                    .map_err(|error| format!("Tauri Channel send failed: {error}"))?;
            }
            Ok(ReadEvent::Eof) => break,
            Ok(ReadEvent::Error(error)) => return Err(format!("PTY read failed: {error}")),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("PTY reader disconnected before EOF".to_string());
            }
        }
    }
    let process_exit_code = wait_success(child.as_mut())?;
    let expected = request.bytes + MARKER.len();
    if streamed != expected {
        return Err(format!(
            "PTY source count mismatch: expected {expected}, streamed {streamed}"
        ));
    }

    Ok(SpikeStreamResult {
        payload_bytes: request.bytes,
        streamed_bytes: streamed,
        marker_bytes: MARKER.len(),
        process_exit_code,
    })
}

#[tauri::command]
pub async fn spike_pty_stream(
    channel: Channel<Vec<u8>>,
    request: SpikeStreamRequest,
) -> Result<SpikeStreamResult, String> {
    tauri::async_runtime::spawn_blocking(move || stream_pty(channel, request))
        .await
        .map_err(|error| format!("PTY worker failed: {error}"))?
}

#[tauri::command]
pub async fn spike_resize(rows: u16, cols: u16) -> Result<SpikeResizeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if rows == 0 || cols == 0 || rows > 500 || cols > 500 {
            return Err("resize dimensions must be between 1 and 500".to_string());
        }
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("openpty failed: {error}"))?;
        let command = if cfg!(unix) {
            let mut command = CommandBuilder::new("bash");
            command.arg("-c");
            command.arg("read ignored; size=$(stty size); echo SIZE=${size/ /x}");
            command
        } else {
            let mut command = CommandBuilder::new("powershell.exe");
            command.arg("-NoProfile");
            command.arg("-Command");
            command.arg("$null=Read-Host; Write-Output ('SIZE={0}x{1}' -f [Console]::WindowHeight,[Console]::WindowWidth)");
            command
        };
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("spawn failed: {error}"))?;
        let receiver = reader_events(
            pair.master
                .try_clone_reader()
                .map_err(|error| format!("clone reader failed: {error}"))?,
        );
        drop(pair.slave);
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("take writer failed: {error}"))?;
        pair.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("resize failed: {error}"))?;
        writer
            .write_all(b"\r")
            .and_then(|()| writer.flush())
            .map_err(|error| format!("resize trigger failed: {error}"))?;
        let output = receive_until_eof(&receiver, child.as_mut())?;
        let process_exit_code = wait_success(child.as_mut())?;
        let expected = format!("SIZE={rows}x{cols}");
        let text = String::from_utf8_lossy(&output);
        if !text.contains(&expected) {
            return Err(format!(
                "child did not observe {expected}; output was {:?}",
                &text[..text.len().min(500)]
            ));
        }
        Ok(SpikeResizeResult {
            requested_rows: rows,
            requested_cols: cols,
            observed_rows: rows,
            observed_cols: cols,
            process_exit_code,
        })
    })
    .await
    .map_err(|error| format!("resize worker failed: {error}"))?
}

#[tauri::command]
pub async fn spike_input_echo(input: String) -> Result<SpikeInputResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if input.is_empty() || input.len() > 1024 || input.contains(['\r', '\n']) {
            return Err("input must be 1-1024 bytes without newlines".to_string());
        }
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("openpty failed: {error}"))?;
        let command = if cfg!(unix) {
            let mut command = CommandBuilder::new("bash");
            command.arg("-c");
            command.arg("read line; printf 'ECHO:%s' \"$line\"");
            command
        } else {
            let mut command = CommandBuilder::new("powershell.exe");
            command.arg("-NoProfile");
            command.arg("-Command");
            command.arg("$line=Read-Host; [Console]::Write('ECHO:'+$line)");
            command
        };
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("spawn failed: {error}"))?;
        let receiver = reader_events(
            pair.master
                .try_clone_reader()
                .map_err(|error| format!("clone reader failed: {error}"))?,
        );
        drop(pair.slave);
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("take writer failed: {error}"))?;
        writer
            .write_all(format!("{input}\r").as_bytes())
            .and_then(|()| writer.flush())
            .map_err(|error| format!("input write failed: {error}"))?;
        let output = receive_until_eof(&receiver, child.as_mut())?;
        let process_exit_code = wait_success(child.as_mut())?;
        let expected = format!("ECHO:{input}");
        let text = String::from_utf8_lossy(&output);
        if !text.contains(&expected) {
            return Err(format!(
                "input echo mismatch; expected {expected:?}, output {:?}",
                &text[..text.len().min(500)]
            ));
        }
        Ok(SpikeInputResult {
            echoed: input,
            process_exit_code,
        })
    })
    .await
    .map_err(|error| format!("input worker failed: {error}"))?
}

fn exit_after_response(app: tauri::AppHandle, code: i32) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        app.exit(code);
        std::process::exit(code);
    });
}

#[tauri::command]
pub fn spike_complete(app: tauri::AppHandle, report: SpikeBrowserReport) -> Result<(), String> {
    let valid = browser_report_is_valid(&report);
    let json = serde_json::to_string(&report).map_err(|error| error.to_string())?;
    println!("M2_REAL_WEBVIEW_REPORT={json}");
    if valid {
        exit_after_response(app, 0);
        Ok(())
    } else {
        eprintln!("M2_REAL_WEBVIEW_INVALID={json}");
        exit_after_response(app, 1);
        Err("real WebView report failed validation".to_string())
    }
}

#[tauri::command]
pub fn spike_fail(app: tauri::AppHandle, message: String) -> Result<(), String> {
    eprintln!("M2_REAL_WEBVIEW_FAILURE={message}");
    exit_after_response(app, 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_report() -> SpikeBrowserReport {
        SpikeBrowserReport {
            payload_bytes: 262_144,
            delivered_payload_bytes: 262_144,
            expected_sha256: "a".repeat(64),
            delivered_sha256: "a".repeat(64),
            exact_byte_integrity: true,
            xterm_write_completed: true,
            input_return: true,
            real_resize: true,
            process_exit_code: 0,
        }
    }

    #[test]
    fn complete_browser_report_passes() {
        assert!(browser_report_is_valid(&valid_report()));
    }

    #[test]
    fn byte_or_pipeline_mismatch_fails() {
        let mut report = valid_report();
        report.delivered_payload_bytes -= 1;
        assert!(!browser_report_is_valid(&report));

        let mut report = valid_report();
        report.xterm_write_completed = false;
        assert!(!browser_report_is_valid(&report));
    }
}
