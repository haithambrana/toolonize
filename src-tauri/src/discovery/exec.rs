//! Desktop Entry `Exec` tokenizer and field-code expander.
//!
//! Implements the specification's quoting rules exactly and never invokes a
//! shell: shell metacharacters are literal argument data (T-DISC-03,
//! T-SEC-002a). Anything the specification leaves undefined, or that a
//! well-formed entry would not contain, is reported as an issue so the
//! launcher lands in Needs Review instead of being guessed at.

const RESERVED: &[char] = &[
    ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')',
    '`',
];
const MAX_ARGS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub quoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tokens {
    pub tokens: Vec<Token>,
    pub issues: Vec<String>,
}

/// Split an (already string-unescaped) Exec value into arguments.
pub fn tokenize(exec: &str) -> Tokens {
    let mut out = Tokens::default();
    let mut chars = exec.chars().peekable();
    loop {
        while chars.peek() == Some(&' ') {
            chars.next();
        }
        let Some(&first) = chars.peek() else { break };
        if out.tokens.len() >= MAX_ARGS {
            out.issues.push(format!("more than {MAX_ARGS} arguments"));
            break;
        }
        if first == '"' {
            chars.next();
            let mut text = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                match c {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => match chars.next() {
                        Some(e @ ('"' | '`' | '$' | '\\')) => text.push(e),
                        Some(other) => {
                            out.issues
                                .push(format!("undefined escape \\{other} inside quotes"));
                            text.push('\\');
                            text.push(other);
                        }
                        None => out.issues.push("dangling backslash".into()),
                    },
                    '`' | '$' => {
                        out.issues
                            .push(format!("unescaped '{c}' inside quoted argument"));
                        text.push(c);
                    }
                    other => text.push(other),
                }
            }
            if !closed {
                out.issues.push("unterminated quoted argument".into());
            }
            if let Some(&next) = chars.peek() {
                if next != ' ' {
                    out.issues
                        .push("quoted argument not followed by a separator".into());
                }
            }
            out.tokens.push(Token { text, quoted: true });
        } else {
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if c == ' ' {
                    break;
                }
                chars.next();
                if RESERVED.contains(&c) {
                    out.issues.push(format!(
                        "reserved character '{}' outside quotes",
                        c.escape_default()
                    ));
                }
                text.push(c);
            }
            out.tokens.push(Token {
                text,
                quoted: false,
            });
        }
    }
    if out.tokens.is_empty() {
        out.issues.push("empty Exec".into());
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct ExpandContext<'a> {
    pub icon: Option<&'a str>,
    pub name: Option<&'a str>,
    pub desktop_file: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Expansion {
    pub argv: Vec<String>,
    pub issues: Vec<String>,
    /// Whether the template accepts files/URLs (%f %F %u %U).
    pub accepts_targets: bool,
}

/// Expand field codes for a launch with no files or URLs.
pub fn expand(tokens: &Tokens, ctx: &ExpandContext<'_>) -> Expansion {
    let mut out = Expansion {
        issues: tokens.issues.clone(),
        ..Default::default()
    };
    let mut target_codes = 0;
    for token in &tokens.tokens {
        if token.quoted {
            if contains_field_code(&token.text) {
                out.issues
                    .push("field code inside a quoted argument (undefined by the spec)".into());
            }
            out.argv.push(token.text.clone());
            continue;
        }
        match token.text.as_str() {
            "%i" => {
                if let Some(icon) = ctx.icon.filter(|i| !i.is_empty()) {
                    out.argv.push("--icon".into());
                    out.argv.push(icon.to_string());
                }
                continue;
            }
            "%f" | "%F" | "%u" | "%U" => {
                target_codes += 1;
                out.accepts_targets = true;
                continue;
            }
            "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => continue,
            _ => {}
        }
        let mut arg = String::new();
        let mut drop_arg = false;
        let mut chars = token.text.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                arg.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => arg.push('%'),
                Some('f' | 'F' | 'u' | 'U') => {
                    target_codes += 1;
                    out.accepts_targets = true;
                    // "--file=%f" with no file: drop the whole argument.
                    drop_arg = true;
                }
                Some('c') => arg.push_str(ctx.name.unwrap_or("")),
                Some('k') => arg.push_str(ctx.desktop_file.unwrap_or("")),
                Some('d' | 'D' | 'n' | 'N' | 'v' | 'm') => {}
                Some('i') => {
                    out.issues.push("%i must be a standalone argument".into());
                }
                Some(other) => out.issues.push(format!("invalid field code %{other}")),
                None => out.issues.push("trailing '%' in Exec".into()),
            }
        }
        if !drop_arg {
            out.argv.push(arg);
        }
    }
    if target_codes > 1 {
        out.issues.push("more than one file/URL field code".into());
    }
    if out.argv.is_empty() {
        out.issues.push("Exec expands to an empty command".into());
    }
    out
}

fn contains_field_code(text: &str) -> bool {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some(n) if "fFuUickdDnNvm".contains(n) => return true,
                _ => {}
            }
        }
    }
    false
}

/// Encode argv per the Exec quoting rules (used by tests and by exports).
pub fn encode(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if !arg.is_empty() && !arg.contains(RESERVED) && !arg.contains('%') {
                arg.clone()
            } else {
                let mut quoted = String::from("\"");
                for c in arg.chars() {
                    if matches!(c, '"' | '`' | '$' | '\\') {
                        quoted.push('\\');
                    }
                    quoted.push(c);
                }
                quoted.push('"');
                quoted
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(exec: &str) -> Expansion {
        expand(&tokenize(exec), &ExpandContext::default())
    }

    #[test]
    fn simple_with_url_code() {
        let e = argv("example-tool --new-window %U");
        assert_eq!(e.argv, vec!["example-tool", "--new-window"]);
        assert!(e.accepts_targets);
        assert!(e.issues.is_empty());
    }

    #[test]
    fn quoted_arguments_and_escapes() {
        let e = argv(r#"tool "a b" "say \"hi\"" "cost \$5" "back\\slash""#);
        assert_eq!(
            e.argv,
            vec!["tool", "a b", "say \"hi\"", "cost $5", "back\\slash"]
        );
        assert!(e.issues.is_empty(), "{:?}", e.issues);
    }

    #[test]
    fn shell_metacharacters_stay_literal_t_sec_002() {
        // Quoted: one literal argument, no shell ever interprets it.
        let e = argv(r#"tool "foo; rm -rf ~ && echo pwned""#);
        assert_eq!(e.argv, vec!["tool", "foo; rm -rf ~ && echo pwned"]);
        // Unquoted reserved characters are malformed: kept literal AND flagged.
        let e = argv("tool foo;whoami |cat");
        assert_eq!(e.argv, vec!["tool", "foo;whoami", "|cat"]);
        assert!(!e.issues.is_empty());
    }

    #[test]
    fn field_codes() {
        let ctx = ExpandContext {
            icon: Some("org.example.Tool"),
            name: Some("Example Tool"),
            desktop_file: Some("/fixture/org.example.Tool.desktop"),
        };
        let e = expand(&tokenize("tool %i --title=%c %k 100%% %d"), &ctx);
        assert_eq!(
            e.argv,
            vec![
                "tool",
                "--icon",
                "org.example.Tool",
                "--title=Example Tool",
                "/fixture/org.example.Tool.desktop",
                "100%"
            ]
        );
        assert!(e.issues.is_empty());
        assert_eq!(argv("tool %i").argv, vec!["tool"]);
    }

    #[test]
    fn undefined_or_invalid_codes_need_review() {
        assert!(!argv(r#"tool "%f""#).issues.is_empty());
        assert!(!argv("tool %x").issues.is_empty());
        assert!(!argv("tool %f %u").issues.is_empty());
        assert!(!argv("tool 100%").issues.is_empty());
        assert!(!argv(r#"tool "unterminated"#).issues.is_empty());
        assert!(!argv(r#"tool "a"b"#).issues.is_empty());
        assert!(!argv(r#"sh -c "echo $HOME""#).issues.is_empty());
        assert!(!argv("").issues.is_empty());
    }

    #[test]
    fn inline_file_code_drops_argument() {
        assert_eq!(argv("tool --file=%f -v").argv, vec!["tool", "-v"]);
    }

    proptest::proptest! {
        #[test]
        fn encode_then_parse_round_trips(args in proptest::collection::vec("[ -$&-~]{0,12}", 1..6)) {
            // Printable ASCII except '%' (field codes are a separate grammar).
            let encoded = encode(&args);
            let parsed = tokenize(&encoded);
            proptest::prop_assert!(parsed.issues.is_empty(), "{:?} -> {} -> {:?}", args, encoded, parsed.issues);
            let texts: Vec<String> = parsed.tokens.into_iter().map(|t| t.text).collect();
            proptest::prop_assert_eq!(texts, args);
        }

        #[test]
        fn tokenizer_never_panics(s in "\\PC{0,200}") {
            let _ = expand(&tokenize(&s), &ExpandContext::default());
        }
    }
}
