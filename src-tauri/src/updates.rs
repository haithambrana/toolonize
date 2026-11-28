//! In-app updates (ADR-019). The core checks GitHub Releases for a newer
//! version and installs it only after the downloaded file's signature
//! matches the public key built into this app; the WebView only asks
//! "check" and "install" and never supplies a URL.
//!
//! The update endpoint and public key are added to release builds by the
//! release workflow. Builds without them (development, self-built) report
//! that updates are not set up, and the updater plugin is not loaded.

use crate::app::Core;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Whether this build carries an updater configuration.
pub struct UpdatesEnabled(pub bool);

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    /// This build can update itself.
    pub configured: bool,
    pub current: String,
    pub available: bool,
    pub version: Option<String>,
    /// Release notes, trimmed.
    pub notes: Option<String>,
    pub date: Option<String>,
}

pub const MAX_NOTES: usize = 4000;

pub fn trim_notes(body: Option<String>) -> Option<String> {
    let body = body?;
    let clean: String = body
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return None;
    }
    Some(if clean.chars().count() > MAX_NOTES {
        format!("{}…", clean.chars().take(MAX_NOTES).collect::<String>())
    } else {
        clean.to_string()
    })
}

fn enabled(app: &AppHandle) -> bool {
    app.try_state::<UpdatesEnabled>().is_some_and(|s| s.0)
}

/// The plugin checks the signature while downloading; say "refused" for a
/// signature or version mismatch rather than "download failed".
pub fn describe_failure(message: &str) -> String {
    let lower = message.to_lowercase();
    if lower.contains("signature") || lower.contains("signed") || lower.contains("tampered") {
        format!("update refused, nothing was installed: {message}")
    } else {
        format!("download failed: {message}")
    }
}

pub fn not_configured(current: String) -> UpdateStatus {
    UpdateStatus {
        configured: false,
        current,
        available: false,
        version: None,
        notes: None,
        date: None,
    }
}

pub async fn check(app: &AppHandle) -> Result<UpdateStatus, String> {
    let current = app.package_info().version.to_string();
    if !enabled(app) {
        return Ok(not_configured(current));
    }
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = match updater.check().await {
        Ok(found) => found,
        // Nothing published yet at the endpoint (HTTP 404 and the like):
        // there is simply no newer release. Network errors stay errors.
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => None,
        Err(e) => return Err(format!("could not check for updates: {e}")),
    };
    crate::logging::info(
        "update",
        &format!(
            "check: current {current}, latest {}",
            found.as_ref().map_or("none", |u| u.version.as_str())
        ),
    );
    Ok(match found {
        Some(u) => UpdateStatus {
            configured: true,
            current,
            available: true,
            version: Some(u.version.clone()),
            notes: trim_notes(u.body.clone()),
            date: u.date.map(|d| d.to_string()),
        },
        None => UpdateStatus {
            configured: true,
            current,
            available: false,
            version: None,
            notes: None,
            date: None,
        },
    })
}

/// Download, verify, install, then restart. Terminals are saved by session
/// memory first and come back after the restart (ADR-015).
pub async fn install(app: AppHandle, core: Arc<Core>) -> Result<(), String> {
    if !enabled(&app) {
        return Err("updates are not set up in this build".into());
    }
    let exit_core = core.clone();
    let updater = app
        .updater_builder()
        // Windows: the installer replaces the app, which then exits.
        .on_before_exit(move || exit_core.shutdown())
        .build()
        .map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("could not check for updates: {e}"))?
        .ok_or("ToolOnize is already up to date")?;
    let version = update.version.clone();
    core.record_update("update.download", &version);
    let mut downloaded: u64 = 0;
    let progress = core.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                progress.update_progress(downloaded, total);
            },
            || {},
        )
        .await
        .map_err(|e| {
            let message = e.to_string();
            core.record_update_failure(&version, &message);
            describe_failure(&message)
        })?;
    // `install` verifies the signature against the built-in public key
    // before anything is written; a mismatch installs nothing.
    update.install(&bytes).map_err(|e| {
        core.record_update_failure(&version, &e.to_string());
        format!("update not installed: {e}")
    })?;
    core.record_update("update.install", &version);
    core.shutdown();
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_are_cleaned_and_bounded() {
        assert_eq!(trim_notes(None), None);
        assert_eq!(trim_notes(Some("  \n ".into())), None);
        assert_eq!(
            trim_notes(Some("Fixes\n\u{1b}[31mred\u{7}\n".into())).as_deref(),
            Some("Fixes\n[31mred")
        );
        let long = "x".repeat(MAX_NOTES + 50);
        let t = trim_notes(Some(long)).unwrap();
        assert_eq!(t.chars().count(), MAX_NOTES + 1);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn signature_problems_are_reported_as_refusals() {
        // Messages as seen in the end-to-end test (wrong key, forged version).
        assert!(describe_failure(
            "The signature was created with a different key than the one provided"
        )
        .starts_with("update refused"));
        assert!(describe_failure(
            "The update was signed for version 0.1.1 but the update endpoint announced version 0.1.3"
        )
        .starts_with("update refused"));
        assert!(describe_failure("connection reset").starts_with("download failed"));
    }

    #[test]
    fn not_configured_reports_only_the_version() {
        let s = not_configured("0.1.0".into());
        assert!(!s.configured && !s.available);
        assert_eq!(s.current, "0.1.0");
        assert!(s.version.is_none() && s.notes.is_none());
    }
}
