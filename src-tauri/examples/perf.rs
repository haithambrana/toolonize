//! Measurement harness for NFR-001/002 (PT-2, PT-4 core path).
//!
//! `cargo run --release --example perf` prints JSON lines. It measures what
//! can be measured without a display: the PTY -> session manager -> acked
//! consumer path (byte-exact, SHA-256 checked) and discovery scan time over
//! synthetic entries. WebView/xterm.js stages and keystroke latency need a
//! real window and are recorded separately (docs/research/perf/).

#[cfg(unix)]
fn main() {
    unix::run();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("perf harness: PTY throughput scenario is Unix-only; run discovery timing via tests on Windows");
}

#[cfg(unix)]
mod unix {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use toolonize_lib::discovery::linux::{scan_applications, LinuxEnv};
    use toolonize_lib::pty::{PortableBackend, PtyDims, SpawnSpec};
    use toolonize_lib::session::{SessionEvent, SessionManager, SessionPlan, SessionSink};

    struct Sink(Mutex<Sender<Vec<u8>>>, AtomicBool);

    impl SessionSink for Sink {
        fn output(&self, frame: Vec<u8>) -> bool {
            self.1.load(Ordering::SeqCst) && self.0.lock().unwrap().send(frame).is_ok()
        }
        fn event(&self, _: &SessionEvent) -> bool {
            true
        }
    }

    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| 32 + (i % 95) as u8).collect()
    }

    fn throughput(len: usize) {
        let manager = SessionManager::new(Arc::new(PortableBackend::new()), Arc::new(|_| {}));
        let script = format!(
            "import sys\nb=bytes(32+(i%95) for i in range(95))*690\nn={len}\nout=sys.stdout.buffer\nwhile n>0:\n    k=min(n,len(b))\n    out.write(b[:k])\n    n-=k\nout.flush()\n"
        );
        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();
        let sink = Arc::new(Sink(Mutex::new(tx), AtomicBool::new(true)));
        let start = Instant::now();
        let info = manager
            .start(SessionPlan {
                workspace_id: "perf".into(),
                member_id: "perf".into(),
                title: "perf".into(),
                spec: SpawnSpec {
                    program: "python3".into(),
                    args: vec!["-c".into(), script],
                    cwd: None,
                    env: vec![],
                },
                remote: false,
                detach_expected: false,
                dims: PtyDims { rows: 24, cols: 80 },
            })
            .unwrap();
        let attach = manager.attach(&info.session_id, 0, sink).unwrap();
        let mut hasher = Sha256::new();
        let mut received = 0usize;
        let mut next = attach.start_offset;
        let mut max_frame = 0usize;
        let mut frames = 0usize;
        while received < len {
            match rx.recv_timeout(Duration::from_secs(30)) {
                Ok(frame) => {
                    let offset = u64::from_le_bytes(frame[..8].try_into().unwrap());
                    assert_eq!(offset, next, "contiguous");
                    let data = &frame[8..];
                    hasher.update(data);
                    received += data.len();
                    next += data.len() as u64;
                    max_frame = max_frame.max(data.len());
                    frames += 1;
                    manager
                        .ack(&info.session_id, attach.attachment_id, next)
                        .unwrap();
                }
                Err(RecvTimeoutError::Timeout) => panic!("stalled at {received}"),
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        let exact = received == len && hasher.finalize()[..] == Sha256::digest(pattern(len))[..];
        println!(
            "{{\"scenario\":\"pty_core_throughput\",\"bytes\":{len},\"seconds\":{elapsed:.3},\"mib_per_s\":{:.1},\"frames\":{frames},\"max_frame\":{max_frame},\"backpressure_events\":{},\"exact_sha256\":{exact}}}",
            len as f64 / elapsed / 1048576.0,
            manager.backpressure_events(&info.session_id).unwrap()
        );
        manager.shutdown();
    }

    fn discovery(entries: usize) {
        let dir = tempfile_dir();
        let apps = dir.join("share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        for i in 0..entries {
            std::fs::write(
                apps.join(format!("org.example.Perf{i}.desktop")),
                format!("[Desktop Entry]\nType=Application\nName=Perf {i}\nExec=perf-tool-{i} --flag \"quoted arg\" %U\nTerminal={}\n", i % 2 == 0),
            )
            .unwrap();
        }
        let env = LinuxEnv {
            home: Some(dir.clone()),
            xdg_data_home: Some(dir.join("none").to_string_lossy().into_owned()),
            xdg_data_dirs: Some(dir.join("share").to_string_lossy().into_owned()),
            ..Default::default()
        };
        let mut runs = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let (found, _) = scan_applications(&env);
            runs.push(start.elapsed().as_secs_f64() * 1000.0);
            assert!(found.len() >= entries);
        }
        println!(
            "{{\"scenario\":\"discovery_scan\",\"entries\":{entries},\"first_ms\":{:.2},\"warm_median_ms\":{:.2}}}",
            runs[0],
            {
                let mut warm = runs[1..].to_vec();
                warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
                warm[warm.len() / 2]
            }
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("toolonize-perf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub fn run() {
        // PT-4 provisional target: 10 MB/s sustained for 30 s = 300 MB.
        throughput(64 * 1024 * 1024);
        throughput(300 * 1000 * 1000);
        discovery(400);
        discovery(2000);
    }
}
