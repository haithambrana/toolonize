//! Workspace Engine data: workspaces, members, layout snapshots, startup
//! policy (FR-020..025). Layout JSON is validated as hostile input
//! (T-CFG-01) before it is stored or loaded.

use crate::model::{now_ms, LauncherType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

pub const MAX_WORKSPACES: usize = 100;
pub const MAX_MEMBERS: usize = 64;
pub const MAX_NAME: usize = 80;
pub const MAX_LAYOUT_BYTES: usize = 256 * 1024;
/// Fixed palette for color tags; the UI maps each id to theme-aware colors.
pub const COLORS: &[&str] = &[
    "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink",
];

/// `None`/empty clears; otherwise an existing absolute folder (no control
/// characters, bounded length). Stored as given.
pub fn validate_dir(dir: Option<&str>) -> Result<Option<String>, String> {
    let Some(dir) = dir.map(str::trim).filter(|d| !d.is_empty()) else {
        return Ok(None);
    };
    if dir.len() > 4096 || dir.chars().any(char::is_control) {
        return Err("folder path is too long or contains control characters".into());
    }
    let path = std::path::Path::new(dir);
    if !path.is_absolute() {
        return Err("use a full folder path (for example /home/me/project)".into());
    }
    if !path.is_dir() {
        return Err(format!("folder does not exist: {dir}"));
    }
    Ok(Some(dir.to_string()))
}

pub const MAX_ENV_VARS: usize = 32;
pub const MAX_ENV_VALUE: usize = 4096;
pub const MAX_STARTUP_COMMAND: usize = 1024;

/// One environment variable set for a terminal member (ADR-016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

/// Per-terminal environment variables and startup command (ADR-016).
/// It only runs once the user confirmed exactly this configuration: a
/// configuration restored from a backup, imported, or changed outside the
/// app waits for confirmation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchConfig {
    #[serde(default)]
    pub env: Vec<EnvVar>,
    /// Typed into the shell once it has started, like the user would.
    #[serde(default)]
    pub startup_command: Option<String>,
    /// Digest of `env` + `startup_command` as the user confirmed them.
    #[serde(default)]
    pub confirmed: Option<String>,
}

impl LaunchConfig {
    pub fn is_empty(&self) -> bool {
        self.env.is_empty() && self.startup_command.is_none()
    }

    pub fn digest(&self) -> String {
        let bytes = serde_json::to_vec(&(&self.env, &self.startup_command))
            .expect("launch config serializes");
        crate::model::hex(&<sha2::Sha256 as sha2::Digest>::digest(bytes))
    }

    pub fn is_confirmed(&self) -> bool {
        self.is_empty() || self.confirmed.as_deref() == Some(self.digest().as_str())
    }

    pub fn confirm(&mut self) {
        self.confirmed = (!self.is_empty()).then(|| self.digest());
    }
}

/// Variables that make the dynamic loader or a shell run extra code, which
/// would bypass what the user sees as the terminal's program. Defense in
/// depth: the real control is explicit confirmation of the configuration.
pub fn env_name_refused(name: &str) -> bool {
    const EXACT: &[&str] = &[
        "BASH_ENV",
        "ENV",
        "PROMPT_COMMAND",
        "PS0",
        "PS4",
        "SHELLOPTS",
        "BASHOPTS",
        "IFS",
        "ZDOTDIR",
        "GCONV_PATH",
        "NODE_OPTIONS",
        "PERL5OPT",
        "RUBYOPT",
        "PYTHONSTARTUP",
        "JAVA_TOOL_OPTIONS",
        "COMSPEC",
        "PATHEXT",
    ];
    let upper = name.to_ascii_uppercase();
    EXACT.contains(&upper.as_str())
        || ["LD_", "DYLD_", "BASH_FUNC_"]
            .iter()
            .any(|p| upper.starts_with(p))
}

/// Check and normalize a launch configuration for a member type.
pub fn validate_launch(
    env: &[EnvVar],
    startup_command: Option<&str>,
    member_type: MemberType,
) -> Result<(Vec<EnvVar>, Option<String>), String> {
    let command = startup_command
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    if member_type == MemberType::ExternalApp && (!env.is_empty() || command.is_some()) {
        return Err("environment variables and startup commands are for terminals only".into());
    }
    if command.is_some()
        && !matches!(
            member_type,
            MemberType::Terminal | MemberType::RemoteCommand
        )
    {
        return Err("a startup command needs a shell or SSH terminal".into());
    }
    if let Some(c) = &command {
        if c.len() > MAX_STARTUP_COMMAND {
            return Err(format!(
                "the startup command can be at most {MAX_STARTUP_COMMAND} characters"
            ));
        }
        if c.chars().any(char::is_control) {
            return Err("the startup command must be a single line".into());
        }
    }
    if env.len() > MAX_ENV_VARS {
        return Err(format!("at most {MAX_ENV_VARS} environment variables"));
    }
    let mut seen = HashSet::new();
    for v in env {
        let name = v.name.as_str();
        let valid = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            return Err(format!(
                "“{name}” is not a valid variable name (letters, digits and _, not starting with a digit)"
            ));
        }
        if env_name_refused(name) {
            return Err(format!(
                "{name} can make programs run extra code, so ToolOnize does not set it"
            ));
        }
        if !seen.insert(name.to_ascii_uppercase()) {
            return Err(format!("{name} is listed twice"));
        }
        if v.value.len() > MAX_ENV_VALUE || v.value.contains('\0') {
            return Err(format!("the value of {name} is too long or contains NUL"));
        }
    }
    Ok((env.to_vec(), command))
}

/// `None` or an empty string clears the tag; anything else must be a palette id.
pub fn validate_color(color: Option<&str>) -> Result<Option<String>, String> {
    match color {
        None | Some("") => Ok(None),
        Some(c) if COLORS.contains(&c) => Ok(Some(c.to_string())),
        Some(_) => Err(format!("color must be one of {}", COLORS.join(", "))),
    }
}
const MAX_LAYOUT_DEPTH: usize = 24;
const MAX_LAYOUT_NODES: usize = 1024;
const MAX_ATTR_STRING: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupPolicy {
    #[default]
    Ask,
    RestoreOnly,
    RestoreTerminals,
    RestoreAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    #[default]
    Grid,
    Focus,
    Tabs,
    MasterStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberType {
    Terminal,
    LocalCommand,
    RemoteCommand,
    ExternalApp,
}

impl MemberType {
    pub fn from_launcher(t: LauncherType) -> Option<Self> {
        match t {
            LauncherType::EmbeddedTerminal => Some(MemberType::Terminal),
            LauncherType::LocalCommand => Some(MemberType::LocalCommand),
            LauncherType::RemoteSsh => Some(MemberType::RemoteCommand),
            LauncherType::ExternalGui => Some(MemberType::ExternalApp),
            LauncherType::Unknown => None,
        }
    }

    pub fn runs_in_pty(self) -> bool {
        !matches!(self, MemberType::ExternalApp)
    }
}

/// Portable description used by non-secret exports (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableHint {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub launcher_id: String,
    pub member_type: MemberType,
    #[serde(default)]
    pub title: Option<String>,
    /// Fingerprint this member was authorized for. `None` = never
    /// authorized (e.g. imported): cannot start.
    #[serde(default)]
    pub authorized_fingerprint: Option<String>,
    #[serde(default)]
    pub authorized_at: Option<u64>,
    #[serde(default)]
    pub auto_restart: bool,
    /// Persistence is delegated to tmux/screen (FR-037), shown in the UI.
    #[serde(default)]
    pub detach_expected: bool,
    pub added_at: u64,
    #[serde(default)]
    pub portable: Option<PortableHint>,
    /// Color tag from [`COLORS`] (UI only; never affects execution).
    #[serde(default)]
    pub color: Option<String>,
    /// Folder a new terminal of this member starts in (split / "new terminal
    /// here"); used only when the launcher defines no working directory.
    #[serde(default)]
    pub start_dir: Option<String>,
    /// Per-terminal font size change in points (-6..=12; 0 = none).
    #[serde(default)]
    pub zoom: i8,
    /// Environment variables and startup command (ADR-016).
    #[serde(default)]
    pub launch: LaunchConfig,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LayoutState {
    pub mode: LayoutMode,
    /// FlexLayout `toJson()` snapshot (validated).
    #[serde(default)]
    pub json: Option<Value>,
    #[serde(default)]
    pub focused_member: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub members: Vec<Member>,
    #[serde(default)]
    pub layout: LayoutState,
    #[serde(default)]
    pub startup_policy: StartupPolicy,
    /// Imported and not yet reviewed (FR-025).
    #[serde(default)]
    pub staged_import: bool,
    /// Color tag from [`COLORS`].
    #[serde(default)]
    pub color: Option<String>,
    /// Project folder: terminals start here unless the launcher or the member
    /// says otherwise (machine-specific, never exported).
    #[serde(default)]
    pub project_dir: Option<String>,
}

impl Workspace {
    pub fn new(id: String, name: String) -> Self {
        let now = now_ms();
        Self {
            id,
            name,
            icon: None,
            created_at: now,
            updated_at: now,
            members: Vec::new(),
            layout: LayoutState::default(),
            startup_policy: StartupPolicy::Ask,
            staged_import: false,
            color: None,
            project_dir: None,
        }
    }

    pub fn member(&self, id: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }

    pub fn member_mut(&mut self, id: &str) -> Option<&mut Member> {
        self.members.iter_mut().find(|m| m.id == id)
    }

    pub fn touch(&mut self) {
        self.updated_at = now_ms();
    }
}

pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("name cannot be empty".into());
    }
    if name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(format!(
            "name must be a single line of at most {MAX_NAME} characters"
        ));
    }
    Ok(name.to_string())
}

pub fn validate_title(title: Option<String>) -> Result<Option<String>, String> {
    match title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    {
        None => Ok(None),
        Some(t) => validate_name(&t).map(Some),
    }
}

fn scalar_ok(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => true,
        Value::String(s) => s.len() <= MAX_ATTR_STRING && !s.chars().any(char::is_control),
        _ => false,
    }
}

struct LayoutCheck<'a> {
    members: &'a HashSet<&'a str>,
    nodes: usize,
    tab_members: Vec<String>,
}

impl LayoutCheck<'_> {
    fn node(&mut self, value: &Value, allowed: &[&str], depth: usize) -> Result<(), String> {
        if depth > MAX_LAYOUT_DEPTH {
            return Err("layout is nested too deeply".into());
        }
        self.nodes += 1;
        if self.nodes > MAX_LAYOUT_NODES {
            return Err("layout has too many nodes".into());
        }
        let obj = value.as_object().ok_or("layout node must be an object")?;
        let kind = obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or("layout node without a type")?;
        if !allowed.contains(&kind) {
            return Err(format!("layout node type '{kind}' is not allowed here"));
        }
        for (key, v) in obj {
            match key.as_str() {
                "children" => {
                    let children = v.as_array().ok_or("children must be an array")?;
                    let child_types: &[&str] = match kind {
                        "row" => &["row", "tabset"],
                        "tabset" | "border" => &["tab", "tabgroup"],
                        "tabgroup" => &["tab"],
                        _ => return Err("tabs cannot have children".into()),
                    };
                    for child in children {
                        self.node(child, child_types, depth + 1)?;
                    }
                }
                "config" if kind == "tab" => {
                    let config = v.as_object().ok_or("tab config must be an object")?;
                    for (ck, cv) in config {
                        if ck != "memberId" || !cv.is_string() {
                            return Err(format!("unsupported tab config key '{ck}'"));
                        }
                    }
                }
                _ if key.len() > 64 => return Err("attribute name too long".into()),
                _ if !scalar_ok(v) => {
                    return Err(format!("attribute '{key}' must be a short scalar"))
                }
                _ => {}
            }
        }
        if kind == "tab" {
            let component = obj.get("component").and_then(Value::as_str);
            match component {
                Some("member") => {
                    let member = obj
                        .get("config")
                        .and_then(|c| c.get("memberId"))
                        .and_then(Value::as_str)
                        .ok_or("member tab without memberId")?;
                    if !self.members.contains(member) {
                        return Err(format!("layout references unknown member '{member}'"));
                    }
                    self.tab_members.push(member.to_string());
                }
                Some("placeholder") => {}
                _ => return Err("tab component must be 'member' or 'placeholder'".into()),
            }
        }
        Ok(())
    }
}

/// Validate a FlexLayout JSON snapshot against this workspace's members.
/// Returns the member ids the layout shows.
pub fn validate_layout(json: &Value, member_ids: &[String]) -> Result<Vec<String>, String> {
    let size = serde_json::to_vec(json).map_err(|e| e.to_string())?.len();
    if size > MAX_LAYOUT_BYTES {
        return Err("layout is too large".into());
    }
    let obj = json.as_object().ok_or("layout must be an object")?;
    let members: HashSet<&str> = member_ids.iter().map(String::as_str).collect();
    let mut check = LayoutCheck {
        members: &members,
        nodes: 0,
        tab_members: Vec::new(),
    };
    for (key, value) in obj {
        match key.as_str() {
            "global" => {
                let global = value.as_object().ok_or("global must be an object")?;
                if !global.values().all(scalar_ok) {
                    return Err("global attributes must be scalars".into());
                }
            }
            "layout" => check.node(value, &["row"], 0)?,
            "borders" => {
                for border in value.as_array().ok_or("borders must be an array")? {
                    check.node(border, &["border"], 0)?;
                }
            }
            "subLayouts" | "popouts" => {
                if !value.as_object().is_some_and(|o| o.is_empty()) {
                    return Err("pop-out windows and floating layouts are not supported".into());
                }
            }
            other => return Err(format!("unknown layout key '{other}'")),
        }
    }
    if !obj.contains_key("layout") {
        return Err("layout root missing".into());
    }
    let mut seen = HashSet::new();
    for m in &check.tab_members {
        if !seen.insert(m.as_str()) {
            return Err(format!("member '{m}' appears twice in the layout"));
        }
    }
    Ok(check.tab_members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tab(member: &str) -> Value {
        json!({"type": "tab", "id": format!("tab-{member}"), "name": member, "component": "member", "config": {"memberId": member}})
    }

    fn sample() -> Value {
        json!({
            "global": {"tabEnableClose": false},
            "borders": [],
            "layout": {"type": "row", "weight": 100, "children": [
                {"type": "tabset", "weight": 50, "children": [tab("m1")]},
                {"type": "row", "children": [
                    {"type": "tabset", "children": [tab("m2"), {"type": "tab", "component": "placeholder", "name": "x"}]}
                ]}
            ]}
        })
    }

    fn ids() -> Vec<String> {
        vec!["m1".into(), "m2".into()]
    }

    #[test]
    fn valid_layout_round_trips_t_layout_002() {
        let shown = validate_layout(&sample(), &ids()).unwrap();
        assert_eq!(shown, vec!["m1", "m2"]);
        let text = serde_json::to_string(&sample()).unwrap();
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back, sample());
    }

    #[test]
    fn hostile_layouts_rejected_t_cfg_01() {
        let mut unknown_node = sample();
        unknown_node["layout"]["children"][0]["type"] = json!("iframe");
        assert!(validate_layout(&unknown_node, &ids()).is_err());

        let mut unknown_member = sample();
        unknown_member["layout"]["children"][0]["children"][0]["config"]["memberId"] =
            json!("ghost");
        assert!(validate_layout(&unknown_member, &ids()).is_err());

        let mut script_component = sample();
        script_component["layout"]["children"][0]["children"][0]["component"] = json!("html");
        assert!(validate_layout(&script_component, &ids()).is_err());

        let mut nested_attr = sample();
        nested_attr["layout"]["children"][0]["evil"] = json!({"a": 1});
        assert!(validate_layout(&nested_attr, &ids()).is_err());

        let mut extra_config = sample();
        extra_config["layout"]["children"][0]["children"][0]["config"]["cmd"] = json!("rm -rf ~");
        assert!(validate_layout(&extra_config, &ids()).is_err());

        let mut popout = sample();
        popout["popouts"] = json!({"p": {"layout": {"type": "row"}}});
        assert!(validate_layout(&popout, &ids()).is_err());

        let mut dup = sample();
        dup["layout"]["children"][1]["children"][0]["children"][0] = tab("m1");
        assert!(validate_layout(&dup, &ids()).is_err());

        assert!(validate_layout(&json!({"bogus": 1, "layout": {"type": "row"}}), &ids()).is_err());
        assert!(validate_layout(&json!({}), &ids()).is_err());

        let mut deep = json!({"type": "row"});
        for _ in 0..40 {
            deep = json!({"type": "row", "children": [deep]});
        }
        assert!(validate_layout(&json!({"layout": deep}), &ids()).is_err());

        let big = "x".repeat(MAX_LAYOUT_BYTES);
        assert!(validate_layout(&json!({"layout": {"type": "row", "name": big}}), &ids()).is_err());
    }

    #[test]
    fn names_validated() {
        assert_eq!(
            validate_name("  Project Phoenix ").unwrap(),
            "Project Phoenix"
        );
        assert!(validate_name("").is_err());
        assert!(validate_name("a\nb").is_err());
        assert!(validate_name(&"x".repeat(MAX_NAME + 1)).is_err());
    }

    #[test]
    fn colors_are_a_fixed_palette() {
        assert_eq!(validate_color(None).unwrap(), None);
        assert_eq!(validate_color(Some("")).unwrap(), None);
        assert_eq!(
            validate_color(Some("teal")).unwrap().as_deref(),
            Some("teal")
        );
        assert!(validate_color(Some("#ff0000")).is_err());
        assert!(validate_color(Some("red;background:url(x)")).is_err());
    }

    #[test]
    fn folders_must_exist_and_be_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_string_lossy().into_owned();
        assert_eq!(validate_dir(Some(&d)).unwrap().as_deref(), Some(d.as_str()));
        assert_eq!(validate_dir(Some("  ")).unwrap(), None);
        assert!(validate_dir(Some("relative/dir")).is_err());
        assert!(validate_dir(Some(&format!("{d}/missing"))).is_err());
        assert!(validate_dir(Some(&format!("{d}\nx"))).is_err());
        let file = dir.path().join("f");
        std::fs::write(&file, b"x").unwrap();
        assert!(validate_dir(Some(&file.to_string_lossy())).is_err());
    }
}
