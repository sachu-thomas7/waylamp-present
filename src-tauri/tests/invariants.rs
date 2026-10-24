// T1.4: CLAUDE.md architecture invariants as tests that read the real repo files, so drift fails.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use streamdeck_lib::config::StreamDeckConfig;
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::slide::SlidePayload;

fn tauri_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    tauri_dir().parent().expect("repo root").to_path_buf()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("list {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            out.extend(rs_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::Null => Vec::new(),
        _ => v
            .as_array()
            .unwrap_or_else(|| panic!("expected array, got {v}"))
            .iter()
            .map(|s| s.as_str().expect("string entry").to_string())
            .collect(),
    }
}

fn capability(name: &str) -> Value {
    read_json(&tauri_dir().join("capabilities").join(name))
}

// --- Rust source sanitiser ------------------------------------------------

fn blank(ch: char) -> char {
    if ch == '\n' {
        '\n'
    } else {
        ' '
    }
}

// Blanks comments and char literals; blanks string contents too unless `keep_strings`. Newlines are preserved.
fn sanitize(src: &str, keep_strings: bool) -> String {
    let c: Vec<char> = src.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    while i < n {
        let ch = c[i];
        let next = c.get(i + 1).copied().unwrap_or('\0');
        if ch == '/' && next == '/' {
            while i < n && c[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if ch == '/' && next == '*' {
            let mut depth = 0;
            while i < n {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(c[i]));
                    i += 1;
                }
            }
        } else if ch == 'r' && (i == 0 || !is_ident(c[i - 1])) && (next == '"' || next == '#') {
            let mut j = i + 1;
            while j < n && c[j] == '#' {
                j += 1;
            }
            if j < n && c[j] == '"' {
                let hashes = j - i - 1;
                let mut k = j + 1;
                while k < n && !(c[k] == '"' && (1..=hashes).all(|h| c.get(k + h) == Some(&'#'))) {
                    k += 1;
                }
                let end = (k + hashes + 1).min(n);
                for &x in &c[i..end] {
                    out.push(if keep_strings { x } else { blank(x) });
                }
                i = end;
            } else {
                out.push(ch);
                i += 1;
            }
        } else if ch == '"' {
            out.push('"');
            i += 1;
            while i < n && c[i] != '"' {
                let step = if c[i] == '\\' && i + 1 < n { 2 } else { 1 };
                for &x in &c[i..(i + step).min(n)] {
                    out.push(if keep_strings { x } else { blank(x) });
                }
                i += step;
            }
            if i < n {
                out.push('"');
                i += 1;
            }
        } else if ch == '\'' {
            if next == '\\' {
                let mut k = i + 2;
                while k < n && c[k] != '\'' {
                    k += 1;
                }
                let end = (k + 1).min(n);
                out.extend(c[i..end].iter().map(|&x| blank(x)));
                i = end;
            } else if c.get(i + 2) == Some(&'\'') {
                out.push_str("   ");
                i += 3;
            } else {
                out.push(ch);
                i += 1;
            }
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

// Removes `#[cfg(test)] mod x { .. }` / `mod x;` items; other `#[cfg(test)]` items (imports) stay visible.
fn strip_test_mods(clean: &str) -> String {
    const ATTR: &str = "#[cfg(test)]";
    let mut text = clean.to_string();
    let mut from = 0;
    while let Some(rel) = text[from..].find(ATTR) {
        let start = from + rel;
        let after = start + ATTR.len();
        let rest = text[after..].trim_start();
        let rest = rest.strip_prefix("pub ").unwrap_or(rest).trim_start();
        if !rest.starts_with("mod ") {
            from = after;
            continue;
        }
        let bytes = text.as_bytes();
        let mut j = after;
        while j < bytes.len() && bytes[j] != b'{' && bytes[j] != b';' {
            j += 1;
        }
        let mut end = j + 1;
        if j < bytes.len() && bytes[j] == b'{' {
            let mut depth = 1;
            while end < bytes.len() && depth > 0 {
                match bytes[end] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                end += 1;
            }
        }
        let end = end.min(text.len());
        let blanked: String = text[start..end].chars().map(blank).collect();
        text = format!("{}{}{}", &text[..start], blanked, &text[end..]);
        from = start;
    }
    text
}

fn production_text(src: &str, keep_strings: bool) -> String {
    strip_test_mods(&sanitize(src, keep_strings))
}

fn production_lines(src: &str) -> Vec<(usize, String)> {
    production_text(src, false)
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.to_string()))
        .collect()
}

fn line_of(text: &str, idx: usize) -> usize {
    text[..idx].matches('\n').count() + 1
}

fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

#[test]
fn sanitizer_self_test() {
    let t = production_text(
        "let a = \"x.unwrap()\"; // y.unwrap()\nlet b = '\"'; /* z */",
        false,
    );
    assert!(!t.contains("unwrap") && !t.contains('z'), "{t}");
    let t = production_text("let r = r#\"a \" .unwrap() \"#; let l: &'a str;", false);
    assert!(!t.contains("unwrap") && t.contains("&'a str"), "{t}");
    let t = production_text("let s = \"keep me\";", true);
    assert!(t.contains("keep me"));
}

// --- no panicking calls in command handlers -------------------------------

const PANIC_METHODS: [&str; 4] = [".unwrap", ".expect", ".unwrap_err", ".expect_err"];
const PANIC_MACROS: [&str; 4] = ["panic!", "todo!", "unimplemented!", "unreachable!"];

fn panicking_calls(src: &str) -> Vec<(usize, String)> {
    let text = production_text(src, false);
    let mut hits = Vec::new();
    for m in PANIC_METHODS {
        for (idx, _) in text.match_indices(m) {
            let next = text[idx + m.len()..].trim_start().chars().next();
            if next == Some('(') {
                hits.push(idx);
            }
        }
    }
    for m in PANIC_MACROS {
        for (idx, _) in text.match_indices(m) {
            let prev = text[..idx].chars().next_back();
            if !prev.is_some_and(is_ident_char) {
                hits.push(idx);
            }
        }
    }
    hits.sort_unstable();
    hits.into_iter()
        .map(|idx| {
            let line = line_of(&text, idx);
            (line, text.lines().nth(line - 1).unwrap_or("").to_string())
        })
        .collect()
}

#[test]
fn command_handlers_never_unwrap_or_expect() {
    let mut files = rs_files(&tauri_dir().join("src").join("commands"));
    assert!(files.len() >= 8, "commands/ scan found only {files:?}");
    files.push(tauri_dir().join("src").join("state.rs"));
    let mut offenders = Vec::new();
    for f in &files {
        for (line, text) in panicking_calls(&read(f)) {
            offenders.push(format!("{}:{line}: {}", f.display(), text.trim()));
        }
    }
    assert!(offenders.is_empty(), "panicking calls: {offenders:#?}");
}

#[test]
fn panicking_call_scanner_ignores_tests_comments_strings_but_sees_real_calls() {
    let src =
        "fn a() { x.unwrap(); }\n// y.unwrap()\n#[cfg(test)]\nmod t { fn b() { z.unwrap(); } }";
    let hits = panicking_calls(src);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, 1);

    assert!(panicking_calls("fn a() { f(); // x.unwrap()\n}").is_empty());
    assert!(panicking_calls("fn a() { let s = \"x.unwrap() panic!()\"; }").is_empty());
    assert!(
        panicking_calls("fn a() { x.unwrap_or(1); x.unwrap_or_else(f); my_panic!(); }").is_empty()
    );

    for bad in [
        "fn a() { x.unwrap_err(); }",
        "fn a() { x.expect_err(\"m\"); }",
        "fn a() { panic!(\"no\"); }",
        "fn a() { todo!(); }",
        "fn a() { unimplemented!(); }",
        "fn a() { unreachable!(); }",
        "fn a() {\n    x\n        .unwrap\n        ();\n}",
        "fn a() {\n    x\n        .expect(\n            \"m\",\n        );\n}",
    ] {
        assert_eq!(panicking_calls(bad).len(), 1, "should flag: {bad}");
    }
}

#[test]
fn gated_cfg_test_import_does_not_hide_the_rest_of_the_file() {
    let src = "#[cfg(test)]\nuse foo::Bar;\nfn a() { x.unwrap(); }\n#[cfg(test)]\nmod t { fn b() { y.unwrap(); } }\nfn c() { z.unwrap(); }";
    let hits = panicking_calls(src);
    assert_eq!(hits.iter().map(|h| h.0).collect::<Vec<_>>(), [3, 6]);
}

// --- app.rs stays gated ---------------------------------------------------

#[test]
fn app_module_is_declared_only_behind_cfg_not_test() {
    let lib = read(&tauri_dir().join("src").join("lib.rs"));
    let lines: Vec<&str> = lib.lines().map(str::trim).collect();
    let decls: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| **l == "mod app;" || **l == "pub mod app;")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(decls.len(), 1, "expected exactly one `mod app;`");
    assert_eq!(
        lines[decls[0] - 1],
        "#[cfg(not(test))]",
        "`mod app;` must directly follow #[cfg(not(test))]"
    );
}

// --- capabilities ---------------------------------------------------------

fn glob_match(pat: &str, s: &str) -> bool {
    match pat.split_once('*') {
        None => pat == s,
        Some((head, tail)) => {
            s.starts_with(head)
                && (0..=s.len() - head.len())
                    .filter(|k| s.is_char_boundary(head.len() + k))
                    .any(|k| glob_match(tail, &s[head.len() + k..]))
        }
    }
}

// Every capability Tauri would load: all files in capabilities/ plus inline objects in tauri.conf.json.
fn all_capabilities() -> Vec<(String, Value)> {
    let mut caps = Vec::new();
    for entry in fs::read_dir(tauri_dir().join("capabilities")).expect("capabilities dir") {
        let p = entry.expect("entry").path();
        if p.is_dir() {
            continue;
        }
        let name = p.file_name().expect("name").to_string_lossy().to_string();
        assert!(
            p.extension().is_some_and(|x| x == "json"),
            "capabilities/{name}: only .json is scanned by this test; other formats would bypass it"
        );
        caps.push((name, read_json(&p)));
    }
    let conf = read_json(&tauri_dir().join("tauri.conf.json"));
    if let Some(inline) = conf["app"]["security"]["capabilities"].as_array() {
        for (i, c) in inline.iter().enumerate() {
            if c.is_object() {
                caps.push((format!("tauri.conf.json#capabilities[{i}]"), c.clone()));
            }
        }
    }
    caps
}

fn cap_targets(cap: &Value) -> Vec<String> {
    let mut t = strings(&cap["windows"]);
    t.extend(strings(&cap["webviews"]));
    t
}

fn cap_permissions(cap: &Value) -> Vec<String> {
    cap["permissions"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| match p {
                    Value::String(s) => s.clone(),
                    other => other["identifier"]
                        .as_str()
                        .unwrap_or("<invalid>")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

// `core:event:default` also contains emit/emit_to, so the projector may only hold these two.
const PRESENTATION_PERMISSIONS: [&str; 2] =
    ["core:event:allow-listen", "core:event:allow-unlisten"];

fn presentation_violations(caps: &[(String, Value)]) -> Vec<String> {
    let mut out = Vec::new();
    for (name, cap) in caps {
        if !cap_targets(cap)
            .iter()
            .any(|t| glob_match(t, "presentation"))
        {
            continue;
        }
        for p in cap_permissions(cap) {
            if !PRESENTATION_PERMISSIONS.contains(&p.as_str()) {
                out.push(format!("{name} grants presentation `{p}`"));
            }
        }
    }
    out
}

#[test]
fn no_capability_grants_presentation_more_than_listen_and_unlisten() {
    let caps = all_capabilities();
    assert!(caps.len() >= 2, "capability scan found {caps:?}");
    let violations = presentation_violations(&caps);
    assert!(violations.is_empty(), "{violations:#?}");
    assert!(
        caps.iter().any(
            |(_, c)| cap_targets(c).contains(&"presentation".to_string())
                && cap_permissions(c) == PRESENTATION_PERMISSIONS
        ),
        "no capability grants presentation listen + unlisten"
    );
}

#[test]
fn presentation_violation_scanner_self_test() {
    use serde_json::json;
    let c = |v: Value| vec![("x".to_string(), v)];
    assert!(presentation_violations(&c(
        json!({"windows": ["presentation"], "permissions": ["core:event:allow-listen"]})
    ))
    .is_empty());
    assert!(presentation_violations(&c(
        json!({"windows": ["controller"], "permissions": ["core:default"]})
    ))
    .is_empty());
    for bad in [
        json!({"windows": ["presentation"], "permissions": ["core:default"]}),
        json!({"windows": ["presentation"], "permissions": ["core:event:default"]}),
        json!({"windows": ["presentation"], "permissions": ["core:event:allow-emit"]}),
        json!({"windows": ["*"], "permissions": ["core:default"]}),
        json!({"windows": ["pres*"], "permissions": ["shell:allow-open"]}),
        json!({"webviews": ["presentation"], "permissions": [{"identifier": "dialog:allow-open"}]}),
        json!({"windows": ["controller", "presentation"], "permissions": ["core:event:allow-listen", "core:window:default"]}),
    ] {
        assert_eq!(presentation_violations(&c(bad.clone())).len(), 1, "{bad}");
    }
    assert!(glob_match("*", "presentation") && !glob_match("controller", "presentation"));
}

#[test]
fn presentation_capability_is_listen_only() {
    let cap = capability("presentation.json");
    assert_eq!(strings(&cap["windows"]), ["presentation"]);
    assert_eq!(
        strings(&cap["permissions"]),
        PRESENTATION_PERMISSIONS,
        "presentation may only listen/unlisten (core:event:default would also grant emit)"
    );
}

#[test]
fn no_shipped_capability_carries_remote_origins_and_dev_origin_is_debug_only() {
    for (name, cap) in all_capabilities() {
        assert!(
            cap.get("remote").is_none(),
            "{name}: remote.urls must not ship"
        );
    }
    let app = read(&tauri_dir().join("src").join("app.rs"));
    let at = app
        .find("DEV_PRESENTATION_CAPABILITY)?")
        .expect("dev capability is added");
    assert!(
        app[..at]
            .rfind("if cfg!(debug_assertions)")
            .is_some_and(|i| at - i < 300),
        "dev capability must sit directly under cfg!(debug_assertions)"
    );
    let json = app
        .lines()
        .find(|l| l.starts_with("const DEV_PRESENTATION_CAPABILITY"))
        .expect("const present");
    for needed in [
        "http://localhost:5174/*",
        "core:event:allow-listen",
        "core:event:allow-unlisten",
    ] {
        assert!(json.contains(needed), "dev capability lacks {needed}");
    }
    for forbidden in ["emit", "core:default", "invoke"] {
        assert!(
            !json.contains(forbidden),
            "dev capability must stay listen-only: {forbidden}"
        );
    }
}

#[test]
fn controller_capability_holds_exactly_the_grants_its_ui_uses() {
    let cap = capability("default.json");
    assert_eq!(
        strings(&cap["permissions"]),
        ["core:default", "controller-commands", "dialog:allow-open"],
        "shell/global-shortcut/create-webview-window were unused by controller JS; add grants only with a caller"
    );
}

fn handler_command_names() -> Vec<String> {
    let src = read(&tauri_dir().join("src").join("state.rs"));
    let list = src
        .split("tauri::generate_handler![")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("generate_handler! list");
    list.lines()
        .filter_map(|l| l.trim().strip_prefix("crate::commands::"))
        .filter_map(|l| {
            l.trim_end_matches(',')
                .rsplit("::")
                .next()
                .map(str::to_string)
        })
        .collect()
}

#[test]
fn only_the_controller_may_invoke_app_commands_and_the_set_matches_the_handler_list() {
    let handlers = handler_command_names();
    assert!(handlers.len() >= 20, "handler scan found {handlers:?}");

    let set: toml::Value = read(
        &tauri_dir()
            .join("permissions")
            .join("controller-commands.toml"),
    )
    .parse()
    .expect("permissions toml");
    let sets = set["set"].as_array().expect("[[set]]");
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0]["identifier"].as_str(), Some("controller-commands"));
    let mut granted: Vec<String> = sets[0]["permissions"]
        .as_array()
        .expect("permissions")
        .iter()
        .map(|p| p.as_str().expect("string").to_string())
        .collect();
    let mut expected: Vec<String> = handlers
        .iter()
        .map(|h| format!("allow-{}", h.replace('_', "-")))
        .collect();
    granted.sort();
    expected.sort();
    assert_eq!(
        granted, expected,
        "permissions/controller-commands.toml must list exactly the commands in state.rs"
    );

    // Without an app manifest every command is open to every local window, including the projector.
    let build_rs = read(&tauri_dir().join("build.rs"));
    assert!(build_rs.contains(".app_manifest(") && build_rs.contains(".commands("));

    for (name, cap) in all_capabilities() {
        let grants_set = cap_permissions(&cap)
            .iter()
            .any(|p| p == "controller-commands");
        let targets_presentation = cap_targets(&cap)
            .iter()
            .any(|t| glob_match(t, "presentation"));
        assert!(
            !(grants_set && targets_presentation),
            "{name} gives the projector the controller's commands"
        );
    }
}

#[test]
fn csp_has_no_script_unsafe_inline() {
    let conf = read_json(&tauri_dir().join("tauri.conf.json"));
    let csp = conf["app"]["security"]["csp"].as_str().expect("csp set");
    let script_src = csp
        .split(';')
        .map(str::trim)
        .find(|d| d.starts_with("script-src"))
        .expect("script-src directive");
    assert!(
        !script_src.contains("unsafe-inline") && !script_src.contains("unsafe-eval"),
        "{script_src}"
    );
    assert!(csp.contains("default-src 'self'"), "{csp}");
}

#[test]
fn devtools_and_shell_are_not_shipped() {
    let cargo: toml::Value = read(&tauri_dir().join("Cargo.toml"))
        .parse()
        .expect("Cargo.toml");
    let deps = cargo["dependencies"].as_table().expect("dependencies");
    assert!(
        !dep_features(&deps["tauri"]).contains(&"devtools".to_string()),
        "tauri devtools feature enables the inspector in release builds"
    );
    assert!(
        !deps.contains_key("tauri-plugin-shell"),
        "shell plugin is unused"
    );
}

#[test]
fn controller_capability_targets_controller_only_and_has_core_default() {
    let cap = capability("default.json");
    assert_eq!(strings(&cap["windows"]), ["controller"]);
    assert!(strings(&cap["permissions"]).contains(&"core:default".to_string()));
    assert!(
        cap.get("remote").is_none(),
        "controller must not allow remote origins"
    );
}

// Returns (literal labels, unextractable call sites). A call site is any use of a window/webview builder type.
fn window_label_scan(src: &str) -> (BTreeSet<String>, Vec<String>) {
    let text = production_text(src, true);
    let mut labels = BTreeSet::new();
    let mut bad = Vec::new();
    // (type, label arg index, call suffixes that take the label as a positional argument)
    for (ty, idx_arg, suffixes) in [
        ("WebviewWindowBuilder", 1usize, &["::new("][..]),
        ("WebviewWindow::builder", 1, &["("][..]),
        ("WebviewBuilder", 0, &["::new("][..]),
    ] {
        for (pos, _) in text.match_indices(ty) {
            let prev = text[..pos].chars().next_back();
            if prev.is_some_and(|c| is_ident_char(c) || c == ':') {
                continue;
            }
            let rest = &text[pos + ty.len()..];
            if rest.chars().next().is_some_and(is_ident_char) {
                continue;
            }
            let t = rest.trim_start();
            if t.starts_with([',', ';', '}']) || t.starts_with("as ") {
                continue; // import
            }
            let line = line_of(&text, pos);
            let Some(suffix) = suffixes.iter().find(|s| rest.starts_with(**s)) else {
                bad.push(format!(
                    "line {line}: {ty} used without a literal-label constructor"
                ));
                continue;
            };
            let args = call_args(&rest[suffix.len()..]);
            match args.get(idx_arg).map(|a| a.trim()) {
                Some(a)
                    if a.len() >= 2
                        && a.starts_with('"')
                        && a.ends_with('"')
                        && !a[1..a.len() - 1].contains('"') =>
                {
                    labels.insert(a[1..a.len() - 1].to_string());
                }
                other => bad.push(format!("line {line}: non-literal window label {other:?}")),
            }
        }
    }
    (labels, bad)
}

// Top-level comma-split arguments of a call, given the text right after its `(`.
fn call_args(after_paren: &str) -> Vec<String> {
    let (mut depth, mut args, mut cur, mut in_str) = (0i32, Vec::new(), String::new(), false);
    let mut chars = after_paren.chars();
    while let Some(ch) = chars.next() {
        if in_str {
            cur.push(ch);
            if ch == '\\' {
                cur.extend(chars.next());
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_str = true;
                cur.push(ch);
            }
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(ch);
            }
            ')' | ']' | '}' if depth == 0 => {
                args.push(cur);
                return args;
            }
            ')' | ']' | '}' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => args.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    args.push(cur);
    args
}

fn dynamic_window_labels() -> (BTreeSet<String>, Vec<String>) {
    let (mut labels, mut bad) = (BTreeSet::new(), Vec::new());
    for f in rs_files(&tauri_dir().join("src")) {
        let (l, b) = window_label_scan(&read(&f));
        labels.extend(l);
        bad.extend(b.into_iter().map(|m| format!("{}: {m}", f.display())));
    }
    (labels, bad)
}

#[test]
fn window_label_scanner_self_test() {
    let ok = "use tauri::{WebviewWindowBuilder, Manager};\nuse tauri::WebviewBuilder;\nfn f() {\n    WebviewWindowBuilder::new(\n        app,\n        \"presentation\",\n        url(a, b),\n    ).build()?;\n    WebviewWindow::builder(app, \"second\", u);\n    WebviewBuilder::new(\"third\", u);\n}";
    let (labels, bad) = window_label_scan(ok);
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(
        labels.into_iter().collect::<Vec<_>>(),
        ["presentation", "second", "third"]
    );
    for dynamic in [
        "fn f() { WebviewWindowBuilder::new(app, label, url); }",
        "fn f() { WebviewWindowBuilder::new(app, format!(\"p{}\", n), url); }",
        "fn f() { WebviewWindowBuilder::from_config(app, cfg); }",
        "fn f() { WebviewWindow::builder(app, &name, url); }",
    ] {
        assert_eq!(window_label_scan(dynamic).1.len(), 1, "{dynamic}");
    }
}

#[test]
fn every_window_label_is_covered_by_some_capability() {
    let conf = read_json(&tauri_dir().join("tauri.conf.json"));
    let mut labels: BTreeSet<String> = conf["app"]["windows"]
        .as_array()
        .expect("app.windows")
        .iter()
        .map(|w| w["label"].as_str().expect("label").to_string())
        .collect();
    let (dynamic, unextractable) = dynamic_window_labels();
    assert!(
        unextractable.is_empty(),
        "window created with a label the scanner cannot read: {unextractable:#?}"
    );
    assert!(
        dynamic.contains("presentation"),
        "scanner must find the dynamic presentation window, found {dynamic:?}"
    );
    labels.extend(dynamic);

    let covers: Vec<String> = all_capabilities()
        .iter()
        .flat_map(|(_, c)| cap_targets(c))
        .collect();
    let missing: Vec<_> = labels
        .iter()
        .filter(|l| !covers.iter().any(|pat| glob_match(pat, l)))
        .collect();
    assert!(
        missing.is_empty(),
        "windows with no capability (zero permissions, no listen()): {missing:?}"
    );
}

// --- event union parity ---------------------------------------------------

fn rust_event_variants() -> Vec<String> {
    let src = read(&tauri_dir().join("src").join("events.rs"));
    let body = src
        .split_once("pub enum AppEvent {")
        .expect("AppEvent enum")
        .1;
    let body = &body[..body.find("\n}").expect("enum end")];
    let (mut depth, mut out) = (0i32, Vec::new());
    for line in body.lines() {
        let t = line.trim();
        if depth == 0 && !t.is_empty() && !t.starts_with("//") && !t.starts_with('#') {
            let name: String = t
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            out.push(name);
        }
        for c in t.chars() {
            match c {
                '{' | '(' => depth += 1,
                '}' | ')' => depth -= 1,
                _ => {}
            }
        }
    }
    out
}

// Removes // and /* */ comments from TS, leaving string/template literals intact.
fn strip_ts_comments(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let (mut out, mut i) = (String::new(), 0);
    while i < c.len() {
        let next = c.get(i + 1).copied().unwrap_or('\0');
        if c[i] == '/' && next == '/' {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if c[i] == '/' && next == '*' {
            i += 2;
            while i < c.len() && !(c[i] == '*' && c.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 2;
        } else if matches!(c[i], '\'' | '"' | '`') {
            let q = c[i];
            out.push(q);
            i += 1;
            while i < c.len() && c[i] != q {
                if c[i] == '\\' && i + 1 < c.len() {
                    out.push(c[i]);
                    i += 1;
                }
                out.push(c[i]);
                i += 1;
            }
            if i < c.len() {
                out.push(q);
                i += 1;
            }
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

// Members of `export type AppEvent = | {..} | {..};` split at depth-0 `|`, ending at the depth-0 `;`.
fn ts_union_members(src: &str) -> Vec<String> {
    let clean = strip_ts_comments(src);
    let body = clean
        .split_once("export type AppEvent =")
        .expect("AppEvent union")
        .1;
    let (mut depth, mut members, mut cur, mut quote) = (0i32, Vec::new(), String::new(), None);
    let mut terminated = false;
    for ch in body.chars() {
        if let Some(q) = quote {
            cur.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => {
                quote = Some(ch);
                cur.push(ch);
            }
            '{' | '(' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            '}' | ')' | ']' => {
                depth -= 1;
                cur.push(ch);
            }
            '|' if depth == 0 => members.push(std::mem::take(&mut cur)),
            ';' if depth == 0 => {
                terminated = true;
                break;
            }
            _ => cur.push(ch),
        }
    }
    assert!(terminated, "AppEvent union has no terminating `;`");
    members.push(cur);
    members
        .into_iter()
        .map(|m| m.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|m| !m.is_empty())
        .collect()
}

fn ts_union_entries(path: &Path) -> Vec<(String, String)> {
    ts_union_members(&read(path))
        .into_iter()
        .map(|m| {
            let name = m
                .split_once("type:")
                .and_then(|x| x.1.trim_start().strip_prefix(['\'', '"']))
                .and_then(|x| x.split(['\'', '"']).next())
                .unwrap_or_else(|| panic!("union member without type literal: {m}"));
            assert!(
                m.contains("payload:"),
                "union member must be {{ type, payload }}: {m}"
            );
            (name.to_string(), m.clone())
        })
        .collect()
}

#[test]
fn ts_union_parser_survives_comments_blank_lines_and_multiline_members() {
    let src = "export type AppEvent =\n  // header\n  | { type: 'A'; payload: null }\n\n  /* note */\n  | {\n      type: \"B\";\n      payload: { x: 'p' | 'q'; y: string | null };\n    }\n  | { type: 'C'; payload: string }; // end\nexport type Other = | { type: 'Z' };";
    let m = ts_union_members(src);
    assert_eq!(m.len(), 3, "{m:?}");
    assert!(m[1].contains("type: \"B\"") && m[1].contains("'p' | 'q'"));
}

#[test]
fn rust_event_parser_finds_every_variant() {
    let v = rust_event_variants();
    for name in [
        "BusLag",
        "SlideUpdate",
        "TimerTick",
        "DeckStart",
        "DeckEnd",
        "DisplaysChanged",
        "DisplayLost",
    ] {
        assert!(v.iter().any(|x| x == name), "{name} missing from {v:?}");
    }
    assert!(v.len() >= 19, "got {v:?}");
}

#[test]
fn app_event_is_adjacently_tagged_type_payload() {
    let src = read(&tauri_dir().join("src").join("events.rs"));
    assert!(src.contains(r#"#[serde(tag = "type", content = "payload")]"#));
    let v = serde_json::to_value(AppEvent::SlideUpdate(SlidePayload::empty())).expect("ser");
    assert_eq!(v["type"], "SlideUpdate");
    assert!(v["payload"].is_object());
    let v = serde_json::to_value(AppEvent::TimerTick { elapsed_secs: 7 }).expect("ser");
    assert_eq!(v["type"], "TimerTick");
    assert_eq!(v["payload"]["elapsed_secs"], 7);
    let v = serde_json::to_value(AppEvent::DisplaysChanged(vec![])).expect("ser");
    assert_eq!(v["payload"], serde_json::json!([]));
}

#[test]
fn unit_variants_serialise_without_a_payload_key() {
    // TS declares `payload: null`; the wire shape omits the key, so consumers must not rely on it.
    for ev in [AppEvent::DeckEnd, AppEvent::DeckStart] {
        let v = serde_json::to_value(&ev).expect("ser");
        assert!(v["type"].is_string());
        assert!(
            v.get("payload").is_none(),
            "unit variant grew a payload key: {v}"
        );
    }
}

#[test]
fn controller_event_union_mirrors_every_rust_variant() {
    let rust: BTreeSet<String> = rust_event_variants().into_iter().collect();
    let ts_list = ts_union_entries(&repo_root().join("controller/src/lib/events.ts"));
    let ts: BTreeSet<String> = ts_list.iter().map(|e| e.0.clone()).collect();
    assert_eq!(
        ts_list.len(),
        ts.len(),
        "duplicate member in controller union"
    );
    assert_eq!(rust, ts, "controller AppEvent union drifted from events.rs");
}

#[test]
fn presentation_event_union_only_contains_real_rust_variants() {
    let rust: BTreeSet<String> = rust_event_variants().into_iter().collect();
    let ts_list = ts_union_entries(&repo_root().join("presentation/src/lib/events.ts"));
    assert!(
        ts_list.len() >= 5,
        "presentation union parse looks truncated: {ts_list:?}"
    );
    let unknown: Vec<_> = ts_list.iter().filter(|e| !rust.contains(&e.0)).collect();
    assert!(
        unknown.is_empty(),
        "presentation union has variants Rust does not emit: {unknown:?}"
    );
}

#[test]
fn presentation_union_variant_shapes_match_controller() {
    let ctrl = ts_union_entries(&repo_root().join("controller/src/lib/events.ts"));
    for (name, text) in ts_union_entries(&repo_root().join("presentation/src/lib/events.ts")) {
        let other = ctrl.iter().find(|e| e.0 == name).expect("in controller");
        assert_eq!(text, other.1, "{name} shape differs");
    }
}

// --- presentation is listen-only in code too ------------------------------

// Anything that can invoke: the core module, plugins, the injected global.
fn forbidden_tauri_refs(src: &str) -> Vec<&'static str> {
    ["@tauri-apps/api/core", "@tauri-apps/plugin-", "__TAURI"]
        .into_iter()
        .filter(|p| src.contains(p))
        .collect()
}

// Value imports/re-exports/dynamic loads of a module named `ipc` (type-only forms are allowed).
fn ipc_value_import_offenders(src: &str) -> Vec<String> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if !matches!(c[i], '\'' | '"' | '`') {
            i += 1;
            continue;
        }
        let q = c[i];
        let mut j = i + 1;
        while j < c.len() && c[j] != q && c[j] != '\n' {
            j += 1;
        }
        let lit: String = c[i + 1..j.min(c.len())].iter().collect();
        let before: String = c[..i].iter().collect();
        i = j + 1;
        let last = lit.rsplit('/').next().unwrap_or("");
        if !matches!(last, "ipc" | "ipc.ts" | "ipc.js") {
            continue;
        }
        let b = before.trim_end();
        if b.ends_with("import(") || b.ends_with("require(") {
            out.push(format!("dynamic load of {lit}"));
        } else if b.ends_with("import") {
            out.push(format!("side-effect import of {lit}"));
        } else if b.ends_with("from") {
            let b = b.trim_end_matches("from");
            let kw = ["import", "export"]
                .iter()
                .filter_map(|k| b.rfind(k).map(|p| (p, k.len())))
                .max()
                .map(|(p, l)| b[p + l..].trim().to_string())
                .unwrap_or_default();
            let type_only = kw.starts_with("type ")
                || (kw.starts_with('{')
                    && kw
                        .trim_matches(|c| c == '{' || c == '}' || c == ' ' || c == '\n')
                        .split(',')
                        .all(|s| s.trim().is_empty() || s.trim().starts_with("type ")));
            if !type_only {
                out.push(format!("value import `{kw}` from {lit}"));
            }
        }
    }
    out
}

#[test]
fn presentation_import_scanners_self_test() {
    assert!(ipc_value_import_offenders("import type { A } from './ipc';\nimport { type B, type C } from \"../lib/ipc\";\nimport {\n  type D,\n} from './ipc';").is_empty());
    for bad in [
        "import { invoke2 } from './ipc';",
        "import {\n  a,\n  b,\n} from \"./ipc\";",
        "import { type A, b } from './ipc';",
        "import * as ipc from './ipc';",
        "import ipc from '../lib/ipc';",
        "import './ipc';",
        "const m = await import('./ipc');",
        "const m = require(\"./ipc\");",
        "export { x } from './ipc';",
    ] {
        assert_eq!(ipc_value_import_offenders(bad).len(), 1, "{bad}");
    }
    assert!(forbidden_tauri_refs("import { listen } from '@tauri-apps/api/event';").is_empty());
    for bad in [
        "import { invoke } from \"@tauri-apps/api/core\";",
        "const m = await import(`@tauri-apps/api/core`);",
        "import {\n  open,\n} from '@tauri-apps/plugin-dialog';",
        "window.__TAURI_INTERNALS__.invoke('x')",
    ] {
        assert!(!forbidden_tauri_refs(bad).is_empty(), "{bad}");
    }
}

#[test]
fn presentation_frontend_never_invokes() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).expect("dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "test") {
                    continue;
                }
                walk(&p, out);
            } else if p
                .extension()
                .is_some_and(|x| ["ts", "tsx", "js", "jsx", "mjs"].iter().any(|e| x == *e))
                && !p.to_string_lossy().contains(".test.")
            {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&repo_root().join("presentation/src"), &mut files);
    assert!(files.len() >= 4, "scan found {files:?}");
    let mut offenders = Vec::new();
    for f in files {
        let src = read(&f);
        // lib/ipc.ts is the (unused) wrapper that owns the core import; nobody else may value-import it.
        if !f.ends_with("lib/ipc.ts") {
            for r in forbidden_tauri_refs(&src) {
                offenders.push(format!("{}: references {r}", f.display()));
            }
        }
        for o in ipc_value_import_offenders(&src) {
            offenders.push(format!("{}: {o}", f.display()));
        }
    }
    assert!(offenders.is_empty(), "presentation invokes: {offenders:#?}");
}

// --- secrets never in TOML ------------------------------------------------

const SECRET_WORDS: [&str; 11] = [
    "token",
    "secret",
    "password",
    "passwd",
    "apikey",
    "api_key",
    "bearer",
    "credential",
    "stream_key",
    "streamkey",
    "auth",
];

// Pointers into the keychain are fine; the secret itself is not.
fn is_keychain_pointer(name: &str) -> bool {
    name.ends_with("_keychain_ref") || name.ends_with("_key_id")
}

fn is_secret_name(name: &str) -> bool {
    let n = name.to_lowercase();
    !is_keychain_pointer(&n) && SECRET_WORDS.iter().any(|w| n.contains(w))
}

fn secret_field_names(src: &str) -> Vec<String> {
    production_lines(src)
        .into_iter()
        .filter_map(|(_, l)| {
            let t = l.trim();
            let rest = t.strip_prefix("pub ")?;
            let (name, _) = rest.split_once(':')?;
            let name = name.trim().to_lowercase();
            is_secret_name(&name).then_some(name)
        })
        .collect()
}

#[test]
fn config_structs_have_no_secret_like_fields() {
    let src = read(&tauri_dir().join("src").join("config.rs"));
    assert!(src.contains("pub struct StreamDeckConfig"));
    assert_eq!(secret_field_names(&src), Vec::<String>::new());
    assert_eq!(
        secret_field_names("pub struct C {\n pub whip_bearer_token: String,\n}"),
        ["whip_bearer_token"],
        "scanner must flag a secret-like field"
    );
    let flagged = secret_field_names(
        "pub struct C {\n pub stream_key: String,\n pub auth: String,\n pub whip_token_keychain_ref: String,\n pub stream_key_id: String,\n}",
    );
    assert_eq!(flagged, ["stream_key", "auth"]);
}

#[test]
fn serialised_default_config_contains_no_secret_keys() {
    let toml_text = toml::to_string(&StreamDeckConfig::default()).expect("serialise");
    for line in toml_text.lines() {
        let key = line.split(['=', '[']).find(|s| !s.trim().is_empty());
        if let Some(k) = key {
            assert!(
                !is_secret_name(k.trim()),
                "default TOML has secret-like key `{k}`:\n{toml_text}"
            );
        }
    }
}

// --- test seams and webview globals stay out of release -------------------

fn dep_features(dep: &toml::Value) -> Vec<String> {
    dep.get("features")
        .and_then(|f| f.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// Non-dev places where `tauri/test` could be enabled.
fn tauri_test_leaks(cargo: &toml::Value) -> Vec<String> {
    let mut tables: Vec<(String, &toml::Value)> = Vec::new();
    for k in ["dependencies", "build-dependencies"] {
        if let Some(t) = cargo.get(k) {
            tables.push((k.to_string(), t));
        }
    }
    if let Some(targets) = cargo.get("target").and_then(|t| t.as_table()) {
        for (cfg, body) in targets {
            for k in ["dependencies", "build-dependencies"] {
                if let Some(t) = body.get(k) {
                    tables.push((format!("target.{cfg}.{k}"), t));
                }
            }
        }
    }
    let mut leaks = Vec::new();
    for (where_, t) in tables {
        for (name, dep) in t.as_table().into_iter().flatten() {
            let is_tauri =
                name == "tauri" || dep.get("package").and_then(|p| p.as_str()) == Some("tauri");
            if is_tauri && dep_features(dep).iter().any(|f| f == "test") {
                leaks.push(format!("[{where_}] {name} enables `test`"));
            }
        }
    }
    for (name, members) in cargo
        .get("features")
        .and_then(|f| f.as_table())
        .into_iter()
        .flatten()
    {
        for m in members
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m.as_str())
        {
            if m == "tauri/test" || m == "tauri?/test" {
                leaks.push(format!("[features] {name} enables {m}"));
            }
        }
    }
    leaks
}

#[test]
fn tauri_test_feature_is_dev_dependency_only() {
    let cargo: toml::Value =
        toml::from_str(&read(&tauri_dir().join("Cargo.toml"))).expect("Cargo.toml");
    assert!(cargo["dependencies"].get("tauri").is_some());
    assert!(
        dep_features(&cargo["dev-dependencies"]["tauri"]).contains(&"test".to_string()),
        "dev-dependencies must enable tauri/test"
    );
    let leaks = tauri_test_leaks(&cargo);
    assert!(leaks.is_empty(), "{leaks:?}");
}

#[test]
fn tauri_test_leak_scanner_self_test() {
    let parse = |s: &str| toml::from_str::<toml::Value>(s).expect("toml");
    assert!(tauri_test_leaks(&parse("[dependencies]\ntauri = { version = \"2\", features = [\"devtools\"] }\n[dev-dependencies]\ntauri = { version = \"2\", features = [\"test\"] }")).is_empty());
    for bad in [
        "[dependencies]\ntauri = { version = \"2\", features = [\"test\"] }",
        "[dependencies.tauri]\nversion = \"2\"\nfeatures = [\"test\"]",
        "[target.'cfg(windows)'.dependencies]\ntauri = { version = \"2\", features = [\"test\"] }",
        "[build-dependencies]\ntauri = { version = \"2\", features = [\"test\"] }",
        "[features]\ne2e = [\"tauri/test\"]",
    ] {
        assert_eq!(tauri_test_leaks(&parse(bad)).len(), 1, "{bad}");
    }
}

fn enables_global_tauri(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.iter().any(|(k, x)| {
            (k == "withGlobalTauri" && x != &Value::Bool(false)) || enables_global_tauri(x)
        }),
        Value::Array(a) => a.iter().any(enables_global_tauri),
        _ => false,
    }
}

#[test]
fn tauri_config_does_not_enable_with_global_tauri() {
    let mut confs = vec![tauri_dir().join("tauri.conf.json")];
    for e in fs::read_dir(tauri_dir()).expect("src-tauri") {
        let p = e.expect("entry").path();
        let n = p.file_name().expect("name").to_string_lossy().to_string();
        if n.starts_with("tauri.") && n.ends_with(".conf.json") && !confs.contains(&p) {
            confs.push(p);
        }
    }
    for p in confs {
        assert!(
            !enables_global_tauri(&read_json(&p)),
            "{}: withGlobalTauri exposes IPC to every page (capability minimalism)",
            p.display()
        );
    }
    assert!(enables_global_tauri(
        &serde_json::json!({"app": {"withGlobalTauri": true}})
    ));
    assert!(!enables_global_tauri(
        &serde_json::json!({"app": {"withGlobalTauri": false}})
    ));
}

// --- event bus is the only emit path (invariant 2) ------------------------

fn emit_calls(src: &str) -> Vec<usize> {
    let text = production_text(src, false);
    text.match_indices(".emit")
        .filter(|(idx, _)| {
            let rest = &text[idx + ".emit".len()..];
            let name: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
            (name.is_empty() || name.starts_with('_'))
                && rest[name.len()..].trim_start().starts_with(['(', ':'])
        })
        .map(|(idx, _)| line_of(&text, idx))
        .collect()
}

#[test]
fn emit_is_only_called_from_the_app_event_bridge() {
    let src_dir = tauri_dir().join("src");
    let files = rs_files(&src_dir);
    assert!(files.len() >= 15, "src scan found {files:?}");
    let (mut offenders, mut bridge_hits) = (Vec::new(), 0);
    for f in files {
        let lines = emit_calls(&read(&f));
        if f == src_dir.join("app.rs") {
            bridge_hits = lines.len();
        } else {
            offenders.extend(lines.iter().map(|l| format!("{}:{l}", f.display())));
        }
    }
    assert!(bridge_hits >= 1, "app.rs event bridge no longer emits");
    assert!(
        offenders.is_empty(),
        "emit outside app.rs bridge: {offenders:#?}"
    );
}

#[test]
fn emit_scanner_self_test() {
    assert_eq!(emit_calls("fn f() { h.emit(\"a\", 1); }").len(), 1);
    assert_eq!(
        emit_calls("fn f() { h.emit_to(\"w\", \"a\", 1); h.emit_filter(a, b, c); }").len(),
        2
    );
    assert!(emit_calls("fn f() { // h.emit(x)\n let s = \".emit(\"; }").is_empty());
    assert!(emit_calls("fn f() { h.emitter; }").is_empty());
}

// --- Regression guards moved from verify.sh greps (CLAUDE.md "Multi-window dev/build wiring") ---

fn presentation_command_code() -> String {
    production_text(
        &read(&tauri_dir().join("src/commands/presentation.rs")),
        true,
    )
}

#[test]
fn presentation_show_is_always_fullscreen_with_no_windowed_fallback() {
    let code = presentation_command_code();
    assert!(
        code.contains("set_fullscreen(true)"),
        "presentation_show must go fullscreen"
    );
    assert!(
        !code.contains("single_display"),
        "windowed single-display fallback was reverted on purpose"
    );
}

#[test]
fn presentation_is_always_on_top_only_with_two_or_more_displays() {
    let code = presentation_command_code();
    assert!(
        !code.contains("set_always_on_top(true)"),
        "unconditional always-on-top blocks Alt+Tab on mirrored rigs"
    );
    assert!(code.contains("set_always_on_top(multi_display)"));
    assert!(
        code.contains(".len() >= 2"),
        "multi_display must mean 2+ displays"
    );
}

#[test]
fn presentation_window_close_is_prevented_and_routed_through_hide() {
    let app = production_text(&read(&tauri_dir().join("src/app.rs")), true);
    let at = app
        .find("label() == \"presentation\"")
        .expect("presentation arm in on_window_event");
    let arm = &app[at..];
    let prevent = arm
        .find("prevent_close()")
        .expect("presentation CloseRequested must prevent_close");
    assert!(
        arm[prevent..].contains("hide_presentation("),
        "OS close must act like Stop Presenting"
    );
}

#[test]
fn dev_and_build_commands_start_both_frontends_from_the_repo_root() {
    let conf = read_json(&tauri_dir().join("tauri.conf.json"));
    for (key, needle) in [
        ("beforeDevCommand", "dev-both.cjs"),
        ("beforeBuildCommand", "merge-presentation-dist.cjs"),
    ] {
        let cmd = &conf["build"][key];
        assert_eq!(
            cmd["cwd"], "..",
            "{key} must run from the repo root (no root package.json)"
        );
        assert!(
            cmd["script"].as_str().unwrap_or_default().contains(needle),
            "{key} must run {needle}"
        );
        assert!(
            repo_root().join("scripts").join(needle).is_file(),
            "scripts/{needle} missing"
        );
    }
}

#[test]
fn presentation_build_uses_a_relative_asset_base() {
    let vite = strip_ts_comments(&read(&repo_root().join("presentation/vite.config.ts")));
    assert!(
        vite.contains("base: \"./\""),
        "presentation assets must resolve once relocated under controller/dist/presentation"
    );
}
