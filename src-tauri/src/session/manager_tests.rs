//! Session manager tests against the real platform PTY (L3 / T-PTY suite).

use super::*;
use crate::pty::PortableBackend;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};

struct TestSink {
    frames: Mutex<Sender<Vec<u8>>>,
    events: Mutex<Vec<SessionEvent>>,
    alive: std::sync::atomic::AtomicBool,
}

impl SessionSink for TestSink {
    fn output(&self, frame: Vec<u8>) -> bool {
        self.alive.load(Ordering::SeqCst) && self.frames.lock().unwrap().send(frame).is_ok()
    }
    fn event(&self, event: &SessionEvent) -> bool {
        self.events.lock().unwrap().push(event.clone());
        true
    }
}

fn sink() -> (Arc<TestSink>, Receiver<Vec<u8>>) {
    let (tx, rx) = mpsc::channel();
    (
        Arc::new(TestSink {
            frames: Mutex::new(tx),
            events: Mutex::new(Vec::new()),
            alive: std::sync::atomic::AtomicBool::new(true),
        }),
        rx,
    )
}

fn manager() -> SessionManager {
    SessionManager::new(Arc::new(PortableBackend::new()), Arc::new(|_| {}))
}

fn plan(member: &str, program: &str, args: &[&str]) -> SessionPlan {
    SessionPlan {
        workspace_id: "w-test".into(),
        member_id: member.into(),
        title: member.into(),
        spec: SpawnSpec {
            program: program.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            cwd: None,
            env: vec![],
        },
        remote: false,
        detach_expected: false,
        dims: PtyDims { rows: 24, cols: 80 },
    }
}

/// Consumes frames like the frontend: verifies offsets are contiguous,
/// acknowledges after "writing", optionally slowly.
struct Consumer {
    rx: Receiver<Vec<u8>>,
    attachment: u64,
    next: u64,
    data: Vec<u8>,
}

impl Consumer {
    fn new(rx: Receiver<Vec<u8>>, info: &AttachInfo) -> Self {
        Self {
            rx,
            attachment: info.attachment_id,
            next: info.start_offset,
            data: Vec::new(),
        }
    }

    fn pump_until(
        &mut self,
        m: &SessionManager,
        id: &str,
        timeout: Duration,
        delay: Duration,
        done: impl Fn(&[u8]) -> bool,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if done(&self.data) {
                return true;
            }
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(frame) => {
                    let offset = u64::from_le_bytes(frame[..8].try_into().unwrap());
                    assert_eq!(offset, self.next, "frames must be contiguous");
                    self.next += (frame.len() - 8) as u64;
                    self.data.extend_from_slice(&frame[8..]);
                    if !delay.is_zero() {
                        std::thread::sleep(delay);
                    }
                    m.ack(id, self.attachment, self.next).unwrap();
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return done(&self.data),
            }
        }
        done(&self.data)
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }
}

fn wait_state(
    m: &SessionManager,
    id: &str,
    timeout: Duration,
    pred: impl Fn(&ProcessState) -> bool,
) -> SessionInfo {
    let deadline = Instant::now() + timeout;
    loop {
        let info = m.info(id).unwrap();
        if pred(&info.process_state) || Instant::now() > deadline {
            return info;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn contains(needle: &'static str) -> impl Fn(&[u8]) -> bool {
    move |data| String::from_utf8_lossy(data).contains(needle)
}

#[cfg(unix)]
mod unix {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn exit_code_is_surfaced_t_pty_010() {
        let m = manager();
        let info = m
            .start(plan("a", "sh", &["-c", "printf 'hello-marker'; exit 3"]))
            .unwrap();
        let state = wait_state(&m, &info.session_id, Duration::from_secs(10), |s| {
            s.is_terminal_state()
        });
        assert_eq!(
            state.process_state,
            ProcessState::Exited {
                code: 3,
                signal: None
            }
        );
        // Output produced before any view attached is replayed in full.
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        assert!(!att.trimmed);
        let mut c = Consumer::new(rx, &att);
        assert!(c.pump_until(
            &m,
            &info.session_id,
            Duration::from_secs(5),
            Duration::ZERO,
            contains("hello-marker")
        ));
    }

    fn pattern(len: usize) -> Vec<u8> {
        // Printable ASCII only, so PTY line discipline does not translate it.
        (0..len).map(|i| 32 + (i % 95) as u8).collect()
    }

    fn generator(len: usize) -> String {
        format!(
            "import sys\nb=bytes(32+(i%95) for i in range(95))\nn={len}\nout=sys.stdout.buffer\nwhile n>0:\n    k=min(n,95*64)\n    out.write((b*64)[:k])\n    n-=k\nout.flush()\n"
        )
    }

    fn assert_exact(data: &[u8], len: usize) {
        assert_eq!(data.len(), len, "exact byte count");
        let expected = Sha256::digest(pattern(len));
        let actual = Sha256::digest(data);
        assert_eq!(expected, actual, "SHA-256 integrity");
    }

    #[test]
    fn high_volume_is_lossless_with_backpressure_t_pty_006_007() {
        let limits = FlowLimits {
            retention: 128 * 1024,
            high_water: 64 * 1024,
            low_water: 16 * 1024,
            window: 32 * 1024,
            batch: 8 * 1024,
        };
        let m = manager().with_limits(limits);
        let len = 4 * 1024 * 1024;
        let script = generator(len);
        let info = m.start(plan("big", "python3", &["-c", &script])).unwrap();
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        let mut c = Consumer::new(rx, &att);
        // Slow consumer: forces the reader to pause repeatedly.
        let ok = c.pump_until(
            &m,
            &info.session_id,
            Duration::from_secs(120),
            Duration::from_micros(300),
            |d| d.len() >= len,
        );
        assert!(ok, "received {} of {len}", c.data.len());
        let state = wait_state(&m, &info.session_id, Duration::from_secs(10), |s| {
            s.is_terminal_state()
        });
        assert!(matches!(
            state.process_state,
            ProcessState::Exited { code: 0, .. }
        ));
        c.pump_until(
            &m,
            &info.session_id,
            Duration::from_millis(500),
            Duration::ZERO,
            |_| false,
        );
        assert_exact(&c.data, len);
        assert!(
            m.backpressure_events(&info.session_id).unwrap() > 0,
            "reader must have paused for the slow view"
        );
    }

    #[test]
    fn concurrent_sessions_are_isolated() {
        let m = Arc::new(manager());
        let len = 512 * 1024;
        let handles: Vec<_> = (0..3)
            .map(|i| {
                let m = m.clone();
                std::thread::spawn(move || {
                    let script = generator(len);
                    let info = m
                        .start(plan(&format!("c{i}"), "python3", &["-c", &script]))
                        .unwrap();
                    let (sink, rx) = sink();
                    let att = m.attach(&info.session_id, 0, sink).unwrap();
                    let mut c = Consumer::new(rx, &att);
                    assert!(c.pump_until(
                        &m,
                        &info.session_id,
                        Duration::from_secs(60),
                        Duration::ZERO,
                        |d| d.len() >= len
                    ));
                    assert_exact(&c.data, len);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn resize_is_observed_by_child_t_pty_002() {
        let m = manager();
        let info = m
            .start(plan(
                "r",
                "sh",
                &[
                    "-c",
                    "echo READY; read x; printf 'SIZE=%s' \"$(stty size)\"",
                ],
            ))
            .unwrap();
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(c.pump_until(
            &m,
            &info.session_id,
            Duration::from_secs(10),
            Duration::ZERO,
            contains("READY")
        ));
        let resized = m.resize(&info.session_id, 40, 120).unwrap();
        assert_eq!((resized.rows, resized.cols), (40, 120));
        m.write(&info.session_id, b"\r").unwrap();
        assert!(
            c.pump_until(
                &m,
                &info.session_id,
                Duration::from_secs(10),
                Duration::ZERO,
                contains("SIZE=40 120")
            ),
            "output: {}",
            c.text()
        );
    }

    #[test]
    fn utf8_round_trip_t_pty_003() {
        let m = manager();
        let info = m
            .start(plan(
                "u",
                "sh",
                &["-c", "echo READY; IFS= read -r x; printf 'GOT[%s]' \"$x\""],
            ))
            .unwrap();
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(c.pump_until(
            &m,
            &info.session_id,
            Duration::from_secs(10),
            Duration::ZERO,
            contains("READY")
        ));
        m.write(&info.session_id, "héllo 世界 🚀 e\u{301}\r".as_bytes())
            .unwrap();
        assert!(
            c.pump_until(
                &m,
                &info.session_id,
                Duration::from_secs(10),
                Duration::ZERO,
                contains("GOT[héllo 世界 🚀 e\u{301}]")
            ),
            "output: {}",
            c.text()
        );
    }

    #[test]
    fn ctrl_c_reaches_foreground_process_t_pty_005() {
        let m = manager();
        let info = m
            .start(plan(
                "i",
                "sh",
                &[
                    "-c",
                    "trap 'echo GOT_INT; exit 7' INT; echo READY; while :; do sleep 0.05; done",
                ],
            ))
            .unwrap();
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(c.pump_until(
            &m,
            &info.session_id,
            Duration::from_secs(10),
            Duration::ZERO,
            contains("READY")
        ));
        m.write(&info.session_id, b"\x03").unwrap();
        let state = wait_state(&m, &info.session_id, Duration::from_secs(10), |s| {
            s.is_terminal_state()
        });
        assert!(
            matches!(state.process_state, ProcessState::Exited { code: 7, .. }),
            "{:?}",
            state.process_state
        );
    }

    fn process_gone(pid: i32) -> bool {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => true,
            Ok(stat) => stat
                .rsplit(')')
                .next()
                .map(|rest| rest.trim_start().starts_with('Z'))
                .unwrap_or(false),
        }
    }

    #[test]
    fn stop_terminates_process_group_t_pty_013() {
        let m = manager();
        let info = m
            .start(plan(
                "k",
                "sh",
                &["-c", "trap '' HUP; sleep 1000 & echo BG=$!; wait"],
            ))
            .unwrap();
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(c.pump_until(
            &m,
            &info.session_id,
            Duration::from_secs(10),
            Duration::ZERO,
            |d| {
                String::from_utf8_lossy(d).contains("BG=")
                    && String::from_utf8_lossy(d).contains('\n')
            }
        ));
        let text = c.text();
        let bg: i32 = text
            .split("BG=")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let shell = info.pid.unwrap() as i32;
        let stopped = m.stop(&info.session_id).unwrap();
        assert_eq!(stopped.process_state, ProcessState::Closed);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !(process_gone(shell) && process_gone(bg)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(process_gone(shell), "shell survived stop");
        assert!(process_gone(bg), "background child survived stop (orphan)");
    }

    #[test]
    fn detach_reattach_is_view_only_t_state_002() {
        let m = manager();
        let info = m
            .start(plan(
                "d",
                "sh",
                &["-c", "echo ONE; read x; echo TWO; read y; echo THREE"],
            ))
            .unwrap();
        let id = info.session_id.clone();
        let (s1, rx1) = sink();
        let att1 = m.attach(&id, 0, s1).unwrap();
        let mut c1 = Consumer::new(rx1, &att1);
        assert!(c1.pump_until(
            &m,
            &id,
            Duration::from_secs(10),
            Duration::ZERO,
            contains("ONE")
        ));
        let pid = m.info(&id).unwrap().pid;
        let generation = m.info(&id).unwrap().generation;

        // Hide / detach (e.g. a renderer reload) never changes process state.
        m.view_event(&id, ViewEvent::Hide).unwrap();
        assert_eq!(m.info(&id).unwrap().process_state, ProcessState::Running);
        m.detach(&id, att1.attachment_id).unwrap();
        let detached = m.info(&id).unwrap();
        assert_eq!(detached.view_state, ViewState::Detached);
        assert_eq!(detached.process_state, ProcessState::Running);

        m.write(&id, b"\r").unwrap();
        std::thread::sleep(Duration::from_millis(300));

        // Reattach from the last acknowledged offset: nothing lost or doubled.
        let (s2, rx2) = sink();
        let att2 = m.attach(&id, c1.next, s2).unwrap();
        assert_eq!(att2.start_offset, c1.next);
        assert!(!att2.trimmed);
        let mut c2 = Consumer::new(rx2, &att2);
        assert!(c2.pump_until(
            &m,
            &id,
            Duration::from_secs(10),
            Duration::ZERO,
            contains("TWO")
        ));
        let after = m.info(&id).unwrap();
        assert_eq!(after.pid, pid, "same process");
        assert_eq!(after.generation, generation, "no respawn");
        assert_eq!(after.process_state, ProcessState::Running);
        assert!(!c2.text().contains("ONE"), "no duplicated output");
        m.write(&id, b"\r").unwrap();
        assert!(c2.pump_until(
            &m,
            &id,
            Duration::from_secs(10),
            Duration::ZERO,
            contains("THREE")
        ));
    }

    #[test]
    fn dead_view_detaches_without_touching_process_fs_07() {
        let m = manager();
        let info = m
            .start(plan(
                "v",
                "sh",
                &["-c", "while :; do echo tick; sleep 0.05; done"],
            ))
            .unwrap();
        let id = info.session_id.clone();
        let (s1, _rx1) = sink();
        m.attach(&id, 0, s1.clone()).unwrap();
        // Renderer crash: the channel stops accepting frames.
        s1.alive.store(false, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(5);
        while m.info(&id).unwrap().attached && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let info = m.info(&id).unwrap();
        assert!(!info.attached);
        assert_eq!(info.process_state, ProcessState::Running);
        m.close(&id).unwrap();
        assert!(m.info(&id).is_err());
    }

    #[test]
    fn restart_spawns_fresh_generation_t_pty_011() {
        let m = manager();
        let info = m
            .start(plan("re", "sh", &["-c", "echo GEN; exit 0"]))
            .unwrap();
        let id = info.session_id.clone();
        wait_state(&m, &id, Duration::from_secs(10), |s| s.is_terminal_state());
        let spec = SpawnSpec {
            program: "sh".into(),
            args: vec!["-c".into(), "echo SECOND; sleep 30".into()],
            cwd: None,
            env: vec![],
        };
        let restarted = m.restart(&id, spec).unwrap();
        assert_eq!(restarted.generation, 2);
        let running = wait_state(&m, &id, Duration::from_secs(5), |s| {
            *s == ProcessState::Running
        });
        assert_eq!(running.process_state, ProcessState::Running);
        let (s, rx) = sink();
        let att = m.attach(&id, 0, s).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(
            c.pump_until(&m, &id, Duration::from_secs(10), Duration::ZERO, |d| {
                let t = String::from_utf8_lossy(d);
                t.contains("GEN") && t.contains("SECOND")
            })
        );
        m.close(&id).unwrap();
    }

    #[test]
    fn spawn_failure_is_isolated_fs_02() {
        let m = manager();
        let ok = m.start(plan("ok", "sh", &["-c", "sleep 30"])).unwrap();
        let bad = m
            .start(plan("bad", "/nonexistent/toolonize-missing-binary", &[]))
            .unwrap();
        let bad = wait_state(&m, &bad.session_id, Duration::from_secs(5), |s| {
            s.is_terminal_state()
        });
        assert!(
            matches!(
                bad.process_state,
                ProcessState::Failed { .. } | ProcessState::Exited { .. }
            ),
            "{:?}",
            bad.process_state
        );
        assert_eq!(
            m.info(&ok.session_id).unwrap().process_state,
            ProcessState::Running
        );
        m.shutdown();
    }

    #[test]
    fn dsr_answered_once_when_no_view_t_pty_004() {
        let m = manager();
        let script = "import os,sys,tty\ntty.setraw(0)\nos.write(1,b'\\x1b[6n')\nr=b''\nwhile not r.endswith(b'R'):\n    r+=os.read(0,1)\nos.write(1,b'REPLY='+r[1:]+b'|')\n";
        let info = m.start(plan("dsr", "python3", &["-c", script])).unwrap();
        let id = info.session_id.clone();
        let state = wait_state(&m, &id, Duration::from_secs(10), |s| s.is_terminal_state());
        assert!(matches!(
            state.process_state,
            ProcessState::Exited { code: 0, .. }
        ));
        let (s, rx) = sink();
        let att = m.attach(&id, 0, s).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(
            c.pump_until(
                &m,
                &id,
                Duration::from_secs(5),
                Duration::ZERO,
                contains("REPLY=[1;1R|")
            ),
            "output: {:?}",
            c.text()
        );
    }

    #[test]
    fn one_live_session_per_member() {
        let m = manager();
        let a = m.start(plan("same", "sh", &["-c", "sleep 30"])).unwrap();
        let b = m.start(plan("same", "sh", &["-c", "sleep 30"])).unwrap();
        assert_eq!(a.session_id, b.session_id);
        assert_eq!(m.list().len(), 1);
        m.shutdown();
        assert!(m.list().is_empty());
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    #[test]
    fn conpty_exit_code_and_output_t_pty_010() {
        let m = manager();
        let info = m
            .start(plan("w", "cmd.exe", &["/c", "echo hello-marker & exit 3"]))
            .unwrap();
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(
            c.pump_until(
                &m,
                &info.session_id,
                Duration::from_secs(20),
                Duration::ZERO,
                contains("hello-marker")
            ),
            "output: {}",
            c.text()
        );
        let state = wait_state(&m, &info.session_id, Duration::from_secs(20), |s| {
            s.is_terminal_state()
        });
        assert!(
            matches!(state.process_state, ProcessState::Exited { code: 3, .. }),
            "{:?}",
            state.process_state
        );
    }

    #[test]
    fn conpty_input_round_trip_t_pty_003() {
        let m = manager();
        let info = m
            .start(plan(
                "p",
                "powershell.exe",
                &[
                    "-NoProfile",
                    "-Command",
                    "[Console]::Write('READY'); $l=[Console]::ReadLine(); [Console]::Write('GOT['+$l+']')",
                ],
            ))
            .unwrap();
        let (sink, rx) = sink();
        let att = m.attach(&info.session_id, 0, sink).unwrap();
        let mut c = Consumer::new(rx, &att);
        assert!(c.pump_until(
            &m,
            &info.session_id,
            Duration::from_secs(30),
            Duration::ZERO,
            contains("READY")
        ));
        m.write(&info.session_id, b"abc\r").unwrap();
        assert!(
            c.pump_until(
                &m,
                &info.session_id,
                Duration::from_secs(30),
                Duration::ZERO,
                contains("GOT[abc]")
            ),
            "output: {}",
            c.text()
        );
        m.shutdown();
    }
}

#[test]
fn write_limits_enforced() {
    let m = manager();
    assert_eq!(m.write("missing", b"x"), Err(SessionError::NotFound));
    let big = vec![b'a'; MAX_WRITE + 1];
    assert_eq!(m.write("missing", &big), Err(SessionError::InputTooLarge));
}
