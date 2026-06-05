//! Accuracy guard: every `nestr …` command shown in the skill docs and the
//! README must resolve to a real subcommand path. Each invocation is extracted
//! from the ```bash fences, reduced to its subcommand path, and checked with
//! `nestr <path> --help` (clap exits 2 on an unknown subcommand; --help exits 0
//! and short-circuits before any config/network, so the test is hermetic).
//!
//! Scope: subcommand *paths* only — not flags or positional values.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Recursively collect every `*.md` file under `dir`.
fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

/// The markdown files whose `nestr` commands are validated.
fn doc_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_md(Path::new("skills"), &mut files);
    let readme = PathBuf::from("README.md");
    if readme.exists() {
        files.push(readme);
    }
    files.sort();
    files
}

/// Split a command line into shell-ish words, honoring single/double quotes.
/// Quote characters are consumed; a quoted span (including its spaces) stays in
/// one token so a quoted argument can never look like a bare word.
fn shell_split(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut single = false;
    let mut double = false;
    for c in s.chars() {
        match c {
            '\'' if !double => {
                single = !single;
                has = true;
            }
            '"' if !single => {
                double = !double;
                has = true;
            }
            c if c.is_whitespace() && !single && !double => {
                if has {
                    tokens.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        tokens.push(cur);
    }
    tokens
}

/// Drop an unquoted trailing `# …` comment from a command line.
fn strip_comment(s: &str) -> String {
    let mut single = false;
    let mut double = false;
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double && (i == 0 || chars[i - 1].is_whitespace()) => {
                return chars[..i].iter().collect();
            }
            _ => {}
        }
    }
    s.to_string()
}

/// A token is "bare" — a candidate subcommand — if it is non-empty, does not
/// start with `-` (which would make it a flag), and contains only lowercase
/// letters, digits, and hyphens (so flags, quoted strings, `<placeholders>`,
/// emails, paths, and redirects all stop the path walk).
fn is_bare_word(t: &str) -> bool {
    !t.is_empty()
        && !t.starts_with('-')
        && t.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

const GLOBAL_VALUE_FLAGS: &[&str] = &["-p", "--profile", "--api-key", "--host", "-o", "--output"];
const GLOBAL_BOOL_FLAGS: &[&str] = &["--yes", "--read-only"];

/// Reduce a `nestr …` command line to its subcommand path (e.g. `["nests","get"]`).
/// Returns `None` if the line is not a `nestr` invocation.
fn derive_path(line: &str) -> Option<Vec<String>> {
    let tokens = shell_split(&strip_comment(line));
    let mut it = tokens.into_iter().peekable();
    if it.next().as_deref() != Some("nestr") {
        return None;
    }
    // Skip leading global flags (and the value of value-taking ones).
    while let Some(tok) = it.peek() {
        if GLOBAL_BOOL_FLAGS.contains(&tok.as_str()) {
            it.next();
        } else if GLOBAL_VALUE_FLAGS.contains(&tok.as_str()) {
            it.next();
            it.next(); // its value
        } else if tok.starts_with("--output=") || tok.starts_with("-o=") {
            it.next();
        } else {
            break;
        }
    }
    let mut path = Vec::new();
    while let Some(tok) = it.peek() {
        if is_bare_word(tok) {
            path.push(it.next().unwrap());
        } else {
            break;
        }
    }
    Some(path)
}

/// Extract `(line number, subcommand path)` for each `nestr` invocation inside the
/// ```bash fences of a markdown document.
fn extract_commands(text: &str) -> Vec<(usize, Vec<String>)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut in_bash = false;
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if trimmed.starts_with("```") {
            if in_bash {
                in_bash = false;
            } else {
                let lang = trimmed.trim_start_matches('`').trim();
                in_bash = matches!(lang, "bash" | "sh" | "shell");
            }
            i += 1;
            continue;
        }
        if in_bash {
            // Join backslash line-continuations into one logical line.
            let start = i;
            let mut logical = lines[i].to_string();
            while logical.trim_end().ends_with('\\') {
                let trimmed_end = logical.trim_end();
                logical = trimmed_end[..trimmed_end.len() - 1].to_string();
                i += 1;
                if i < lines.len() {
                    logical.push(' ');
                    logical.push_str(lines[i]);
                } else {
                    break;
                }
            }
            if let Some(path) = derive_path(logical.trim()) {
                if !path.is_empty() {
                    out.push((start + 1, path));
                }
            }
        }
        i += 1;
    }
    out
}

#[test]
fn derive_path_handles_real_shapes() {
    assert_eq!(
        derive_path("nestr nests get <id>"),
        Some(vec!["nests".into(), "get".into()])
    );
    assert_eq!(
        derive_path("nestr search \"quarterly review\""),
        Some(vec!["search".into()])
    );
    assert_eq!(derive_path("nestr -p prod me"), Some(vec!["me".into()]));
    assert_eq!(
        derive_path("nestr -o json nests get <id>"),
        Some(vec!["nests".into(), "get".into()])
    );
    assert_eq!(
        derive_path("nestr tensions vote <a> <b> accept"),
        Some(vec!["tensions".into(), "vote".into()])
    );
    assert_eq!(
        derive_path("nestr users add p@example.com --full-name X"),
        Some(vec!["users".into(), "add".into()])
    );
    assert_eq!(
        derive_path("nestr export work > work.json"),
        Some(vec!["export".into(), "work".into()])
    );
    assert_eq!(derive_path("echo hello"), None);
}

#[test]
fn strip_comment_only_trims_unquoted_hash() {
    assert_eq!(strip_comment("nestr work   # open work"), "nestr work   ");
    assert_eq!(
        strip_comment("nestr search \"a # b\""),
        "nestr search \"a # b\""
    );
}

#[test]
fn extract_skips_non_bash_and_comment_lines() {
    let md = "intro\n\n```bash\nnestr nests get <id>   # read\n# a comment line\n```\n\n```text\nnestr not real\n```\n";
    let cmds = extract_commands(md);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].1, vec!["nests".to_string(), "get".to_string()]);
}

#[test]
fn skill_docs_only_reference_real_commands() {
    let bin = env!("CARGO_BIN_EXE_nestr");
    let mut found: Vec<(PathBuf, usize, Vec<String>)> = Vec::new();
    for file in doc_files() {
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        for (lineno, path) in extract_commands(&text) {
            found.push((file.clone(), lineno, path));
        }
    }
    assert!(
        !found.is_empty(),
        "no nestr commands found in docs — extractor is broken"
    );

    let unique: BTreeSet<Vec<String>> = found.iter().map(|(_, _, p)| p.clone()).collect();
    let mut bad: BTreeSet<Vec<String>> = BTreeSet::new();
    for path in &unique {
        let ok = Command::new(bin)
            .args(path)
            .arg("--help")
            .output()
            .unwrap_or_else(|e| panic!("spawn nestr: {e}"))
            .status
            .success();
        if !ok {
            bad.insert(path.clone());
        }
    }
    if !bad.is_empty() {
        let mut msg = String::from("Docs reference commands that don't resolve:\n");
        for (file, lineno, path) in &found {
            if bad.contains(path) {
                msg.push_str(&format!(
                    "  {}:{}  ->  nestr {} --help (exit != 0)\n",
                    file.display(),
                    lineno,
                    path.join(" ")
                ));
            }
        }
        panic!("{msg}");
    }
}
