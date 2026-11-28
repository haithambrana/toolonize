//! T-DISC-LNX-* fixture tests (fixtures under tests/fixtures/linux, fictional).

use super::*;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/linux")
}

fn env() -> LinuxEnv {
    let f = fixtures().join("desktop-entries");
    LinuxEnv {
        home: Some(fixtures().join("desktop-dir/home")),
        xdg_data_home: Some(f.join("data-home").to_string_lossy().into_owned()),
        xdg_data_dirs: Some(format!(
            "{}:{}",
            f.join("data-system").display(),
            f.join("flatpak/exports/share").display()
        )),
        xdg_config_home: Some(
            fixtures()
                .join("desktop-dir/config")
                .to_string_lossy()
                .into_owned(),
        ),
        current_desktop: Some("GNOME".into()),
        locale: Some("de_DE.UTF-8".into()),
        path: Some("/usr/bin:/bin".into()),
    }
}

fn scan() -> Vec<RawLauncher> {
    scan_applications(&env()).0
}

fn by_id<'a>(all: &'a [RawLauncher], id: &str) -> &'a RawLauncher {
    all.iter()
        .find(|l| l.launcher_id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

#[test]
fn precedence_user_shadows_system_t_disc_lnx_001() {
    let all = scan();
    let tool = by_id(&all, "linux:app:org.example.Tool.desktop");
    assert_eq!(tool.origin, OriginKind::XdgUser);
    assert_eq!(tool.display_name, "Example Tool (user copy)");
    assert_eq!(
        all.iter()
            .filter(|l| l.launcher_id == "linux:app:org.example.Tool.desktop")
            .count(),
        1
    );
}

#[test]
fn normalized_record_shape_t_disc_lnx_002() {
    let all = scan();
    let editor = by_id(&all, "linux:app:org.example.Editor.desktop");
    assert_eq!(editor.display_name, "Beispiel-Editor", "localized Name");
    assert_eq!(editor.platform, Platform::Linux);
    assert_eq!(editor.origin, OriginKind::XdgSystem);
    assert_eq!(editor.parse_status, ParseStatus::Ok);
    assert_eq!(
        editor.descriptor.program.as_deref(),
        Some("/opt/example/bin/example-editor")
    );
    assert_eq!(editor.descriptor.args, vec!["--new-window"]);
    assert!(!editor.descriptor.path_resolved);
    assert!(!editor.descriptor.terminal);
    assert_eq!(
        editor.descriptor.icon.as_deref(),
        Some("org.example.Editor")
    );
    assert_eq!(
        editor.descriptor.exec_template.as_deref(),
        Some("/opt/example/bin/example-editor --new-window %F")
    );
    assert!(editor.relative_path.ends_with("org.example.Editor.desktop"));
    let nested = by_id(&all, "linux:app:vendor-org.example.Nested.desktop");
    assert_eq!(nested.descriptor.program.as_deref(), Some("nested-example"));
    assert!(
        nested.descriptor.path_resolved,
        "bare name resolves via PATH"
    );
}

#[test]
fn hidden_tombstone_removes_system_entry_t_disc_lnx_003() {
    let all = scan();
    assert!(all
        .iter()
        .all(|l| l.launcher_id != "linux:app:org.example.Hidden.desktop"));
}

#[test]
fn nodisplay_and_onlyshowin_kept_but_flagged_t_disc_lnx_004() {
    let all = scan();
    assert!(by_id(&all, "linux:app:org.example.Helper.desktop").no_display);
    assert!(by_id(&all, "linux:app:org.example.KdeOnly.desktop").not_shown_here);
    assert!(!by_id(&all, "linux:app:org.example.Editor.desktop").not_shown_here);
}

#[test]
fn flatpak_origin_tagging_t_disc_lnx_005() {
    let all = scan();
    assert_eq!(
        by_id(&all, "linux:app:org.example.Flat.desktop").origin,
        OriginKind::Flatpak
    );
}

#[test]
fn malformed_corpus_needs_review_never_panics_t_disc_lnx_007() {
    let all = scan();
    for name in [
        "malformed-org.example.HostileQuoting.desktop",
        "malformed-org.example.DuplicateGroup.desktop",
        "malformed-org.example.ControlChars.desktop",
        "malformed-org.example.BadUtf8.desktop",
        "malformed-org.example.Giant.desktop",
    ] {
        let l = by_id(&all, &format!("linux:app:{name}"));
        assert_eq!(l.parse_status, ParseStatus::NeedsReview, "{name}");
        assert!(!l.issues.is_empty(), "{name}");
    }
    let link = by_id(&all, "linux:app:org.example.Link.desktop");
    assert_eq!(link.parse_status, ParseStatus::NeedsReview);
    let dbus = by_id(&all, "linux:app:org.example.Dbus.desktop");
    assert_eq!(dbus.descriptor.kind, DescriptorKind::DbusActivatable);
    assert_eq!(dbus.parse_status, ParseStatus::NeedsReview);
}

#[test]
fn tryexec_missing_is_unavailable() {
    let all = scan();
    let missing = by_id(&all, "linux:app:org.example.Missing.desktop");
    assert_eq!(missing.parse_status, ParseStatus::Unavailable);
}

#[test]
fn path_is_not_enumerated_t_disc_lnx_011() {
    // Only .desktop files from declared roots appear: nothing from PATH.
    let all = scan();
    assert!(all
        .iter()
        .all(|l| l.launcher_id.starts_with("linux:app:") && l.relative_path.ends_with(".desktop")));
}

#[test]
fn user_dirs_relocated_desktop_t_disc_lnx_des_001() {
    let env = env();
    let dir = desktop_dir(&env).expect("desktop dir resolved");
    assert_eq!(dir, fixtures().join("desktop-dir/home/Bureau"));
    let home = Path::new("/home/example");
    assert_eq!(
        parse_user_dirs(
            "XDG_DESKTOP_DIR=\"$HOME/Schreibtisch\"\n",
            home,
            "XDG_DESKTOP_DIR"
        ),
        Some(home.join("Schreibtisch"))
    );
    assert_eq!(
        parse_user_dirs("XDG_DESKTOP_DIR=\"/srv/desk\"\n", home, "XDG_DESKTOP_DIR"),
        Some(PathBuf::from("/srv/desk"))
    );
    // Disabled, missing, relative, or shell-looking values: no Desktop source.
    for text in [
        "XDG_DESKTOP_DIR=\"$HOME\"\n",
        "",
        "XDG_DESKTOP_DIR=\"Desktop\"\n",
        "XDG_DESKTOP_DIR=\"$(whoami)\"\n",
        "XDG_DESKTOP_DIR=\"$HOME/../escape\"\n",
    ] {
        assert_eq!(
            parse_user_dirs(text, home, "XDG_DESKTOP_DIR"),
            None,
            "{text}"
        );
    }
    // Without user-dirs.dirs there is no hard-coded ~/Desktop fallback.
    let mut no_config = env.clone();
    no_config.xdg_config_home = Some("/nonexistent-toolonize-fixture".into());
    assert_eq!(desktop_dir(&no_config), None);
}

#[test]
fn desktop_dir_entries_identity_and_provenance_t_disc_lnx_des_002_003() {
    let env = env();
    let dir = desktop_dir(&env).unwrap();
    let (found, report) = scan_loose_root(&env, &dir, OriginKind::DesktopDir, CUSTOM_LIMITS);
    assert_eq!(report.entries, 2);
    let deploy = found
        .iter()
        .find(|l| l.display_name == "Deploy Helper")
        .unwrap();
    assert_eq!(deploy.origin, OriginKind::DesktopDir);
    assert_eq!(
        deploy.launcher_id, "linux:desktop:Org.Example.DeployHelper.desktop",
        "identity is root + relative path, not a Desktop File ID"
    );
    assert!(deploy.descriptor.terminal);
}

#[test]
fn custom_root_scan_is_bounded_and_labeled_t_disc_lnx_cr_002_004() {
    let env = env();
    let root = fixtures().join("custom-root");
    let (found, report) = scan_loose_root(&env, &root, OriginKind::CustomRoot, CUSTOM_LIMITS);
    assert_eq!(report.entries, 2);
    assert!(found.iter().all(|l| l.origin == OriginKind::CustomRoot));
    assert!(found
        .iter()
        .all(|l| l.launcher_id.starts_with("linux:custom:")));
    let quoted = found
        .iter()
        .find(|l| l.display_name == "Quoted Example")
        .unwrap();
    assert_eq!(quoted.descriptor.args, vec!["foo; rm -rf ~"]);

    let tight = WalkLimits {
        max_depth: 1,
        ..CUSTOM_LIMITS
    };
    let (found, report) = scan_loose_root(&env, &root, OriginKind::CustomRoot, tight);
    assert!(found.is_empty());
    assert!(report.truncated);
}

#[cfg(unix)]
#[test]
fn custom_root_symlink_escape_flagged_t_disc_lnx_cr_003() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("org.example.Outside.desktop"),
        "[Desktop Entry]\nType=Application\nName=Outside\nExec=outside\n",
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("org.example.Outside.desktop"),
        root.path().join("org.example.Outside.desktop"),
    )
    .unwrap();
    let (found, _) = scan_loose_root(&env(), root.path(), OriginKind::CustomRoot, CUSTOM_LIMITS);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].parse_status, ParseStatus::NeedsReview);
    assert!(found[0].issues[0].contains("outside"));
    assert_eq!(found[0].descriptor.program, None, "target content not read");
}

#[test]
fn custom_root_without_opt_in_is_not_scanned_t_disc_lnx_cr_001() {
    // Registered-source scanning never visits custom roots.
    let all = scan();
    assert!(all.iter().all(|l| l.origin != OriginKind::CustomRoot));
}

#[test]
fn application_roots_order() {
    let roots = application_roots(&env());
    assert_eq!(roots[0].origin, OriginKind::XdgUser);
    assert!(roots.iter().any(|r| r.origin == OriginKind::Flatpak));
}
