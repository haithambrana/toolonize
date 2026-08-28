use spike_pty::backends::{all_backends, ScenarioResult, SpikeReport};
use spike_pty::harness;
use spike_pty::transport;
use std::collections::HashMap;
use std::time::Instant;

fn run_and_record(
    results: &mut Vec<ScenarioResult>,
    label: &str,
    run: impl FnOnce() -> ScenarioResult,
) {
    println!("START: {label}");
    let result = run();
    println!(
        "[{}] {}: {} - {} ({}ms)",
        result.backend, result.scenario, result.status, result.details, result.duration_ms
    );
    results.push(result);
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    println!("=== M2 PTY Backend Technical Spike ===");
    println!(
        "Platform: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "Candidates: (a) portable-pty 0.9.0 + mitigations, (b) direct native (openpty/ConPTY)"
    );
    println!();

    let mut all_results: Vec<ScenarioResult> = Vec::new();
    let start_all = Instant::now();

    // Test each backend
    for mut backend in all_backends() {
        let name = backend.name().to_string();
        println!("--- Backend: {} ---", name);

        run_and_record(&mut all_results, "spawn shells", || {
            harness::scenario_spawn_shell(backend.as_mut())
        });
        run_and_record(&mut all_results, "invalid executable", || {
            harness::scenario_invalid_exe(backend.as_mut())
        });
        run_and_record(&mut all_results, "resize", || {
            harness::scenario_resize(backend.as_mut())
        });
        run_and_record(&mut all_results, "UTF-8", || {
            harness::scenario_utf8(backend.as_mut())
        });
        run_and_record(&mut all_results, "Ctrl+C", || {
            harness::scenario_ctrlc(backend.as_mut())
        });
        run_and_record(&mut all_results, "cursor DSR", || {
            harness::scenario_cursor_dsr(backend.as_mut())
        });
        run_and_record(&mut all_results, "high-volume lossless", || {
            harness::scenario_high_volume(backend.as_mut())
        });
        run_and_record(&mut all_results, "cleanup", || {
            harness::scenario_cleanup(backend.as_mut())
        });
        run_and_record(&mut all_results, "TUI", || {
            harness::scenario_tui(backend.as_mut())
        });
        run_and_record(&mut all_results, "agent CLI", || {
            harness::scenario_agent_cli(backend.as_mut())
        });
        run_and_record(&mut all_results, "shell variants", || {
            harness::scenario_shell_variants(backend.as_mut())
        });
        run_and_record(&mut all_results, "hidden console", || {
            harness::scenario_hidden_console(backend.as_mut())
        });
        run_and_record(&mut all_results, "clipboard", || {
            harness::scenario_clipboard(backend.as_mut())
        });

        // Concurrent is separate because it needs fresh backend per session
        run_and_record(&mut all_results, "concurrent sessions", || {
            harness::scenario_concurrent(name.as_str())
        });

        println!();
    }

    // Transport experiment
    println!("--- Bounded LOSSLESS Transport Experiment ---");
    let (lossless_stats, dropping_stats) = transport::run_experiment(2 * 1024 * 1024);
    println!("Lossless: produced {} delivered {} dropped {} max_depth {} backpressure {} breaches {} lossless {}",
        lossless_stats.produced_bytes, lossless_stats.delivered_bytes, lossless_stats.dropped_bytes,
        lossless_stats.max_queue_depth, lossless_stats.backpressure_events, lossless_stats.hard_limit_breaches, lossless_stats.lossless);
    println!(
        "Dropping: produced {} delivered {} dropped {} lossless {}",
        dropping_stats.produced_bytes,
        dropping_stats.delivered_bytes,
        dropping_stats.dropped_bytes,
        dropping_stats.lossless
    );
    let transport_status = if lossless_stats.lossless && lossless_stats.hard_limit_breaches == 0 {
        "PASS"
    } else {
        "FAIL"
    };
    println!(
        "Transport lossless {} - dropping demonstrates why not to use silent drop",
        transport_status
    );
    all_results.push(ScenarioResult {
        backend: "transport".to_string(),
        scenario: "T-PTY-007 backpressure/desync".to_string(),
        status: transport_status.to_string(),
        details: format!(
            "capacity 65536 high_water 49152 low_water 16384; produced {} delivered {} dropped {} backpressure {} max_depth {} hard_breaches {}; dropping dropped {}",
            lossless_stats.produced_bytes,
            lossless_stats.delivered_bytes,
            lossless_stats.dropped_bytes,
            lossless_stats.backpressure_events,
            lossless_stats.max_queue_depth,
            lossless_stats.hard_limit_breaches,
            dropping_stats.dropped_bytes
        ),
        duration_ms: 0,
        extra: {
            let mut m = HashMap::new();
            m.insert(
                "lossless_produced".to_string(),
                lossless_stats.produced_bytes.to_string(),
            );
            m.insert(
                "lossless_delivered".to_string(),
                lossless_stats.delivered_bytes.to_string(),
            );
            m.insert(
                "lossless_dropped".to_string(),
                lossless_stats.dropped_bytes.to_string(),
            );
            m.insert(
                "dropping_dropped".to_string(),
                dropping_stats.dropped_bytes.to_string(),
            );
            m.insert(
                "backpressure_events".to_string(),
                lossless_stats.backpressure_events.to_string(),
            );
            m.insert(
                "max_queue_depth".to_string(),
                lossless_stats.max_queue_depth.to_string(),
            );
            m.insert(
                "hard_limit_breaches".to_string(),
                lossless_stats.hard_limit_breaches.to_string(),
            );
            m
        },
    });

    println!("\n--- Performance Measurement (256 KiB payload) ---");
    for mut backend in all_backends() {
        let name = backend.name().to_string();
        let perf_start = Instant::now();
        let result = harness::scenario_high_volume(backend.as_mut());
        let elapsed = perf_start.elapsed();
        println!(
            "[{}] perf high-volume 256KiB: {} in {:?} ({:.2} MB/s)",
            name,
            result.status,
            elapsed,
            0.25 / elapsed.as_secs_f64()
        );
        all_results.push(ScenarioResult {
            backend: name.clone(),
            scenario: "PERF high-volume 256KiB".to_string(),
            status: result.status.clone(),
            details: result.details.clone(),
            duration_ms: elapsed.as_millis(),
            extra: HashMap::new(),
        });
    }

    // Summary
    println!("\n=== SUMMARY ===");
    let pass = all_results.iter().filter(|r| r.status == "PASS").count();
    let fail = all_results.iter().filter(|r| r.status == "FAIL").count();
    let blocked = all_results.iter().filter(|r| r.status == "BLOCKED").count();
    let not_verified = all_results
        .iter()
        .filter(|r| r.status == "NOT_VERIFIED")
        .count();
    println!(
        "Total: {}, PASS: {}, FAIL: {}, BLOCKED: {}, NOT_VERIFIED: {}",
        all_results.len(),
        pass,
        fail,
        blocked,
        not_verified
    );
    println!("Total duration: {:?}", start_all.elapsed());

    // Write JSON report for CI
    let report_path = "target/spike-report.json";
    std::fs::create_dir_all("target")?;
    let report = SpikeReport {
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        results: all_results,
    };
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(report_path, &json)?;
    println!("Report written to {}", report_path);

    // Also write to docs/research/spike-m2 if exists, else docs/research/PTY_SPIKE_REPORT.json
    let alt_path = "docs/research/spike-m2/report.json";
    if let Some(parent) = std::path::Path::new(alt_path).parent() {
        let _ = std::fs::create_dir_all(parent);
        let _ = std::fs::write(alt_path, &json);
        println!("Also written to {}", alt_path);
    }

    if fail > 0 {
        println!(
            "SPIKE: Some scenarios FAILED - see details above. Decision requires human review."
        );
        std::process::exit(1);
    } else {
        println!("SPIKE: All runnable scenarios PASS on this platform.");
        Ok(())
    }
}
