use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Default, PartialEq)]
pub enum State {
    #[default]
    Untriaged,
    Ignored,
    Accepted,
}

#[derive(Debug, Default, PartialEq)]
pub struct Commit {
    pub url: String,
    pub authors: Vec<String>,
    pub title: String,
    pub hash: String,
    pub hints: Vec<String>,
    pub body: Vec<String>,
    pub date: String,
    pub state: State,
    pub label: String,
}

pub fn write_to_file(commits: Vec<Commit>, path: &Path) -> Result<(), ()> {
    let contents = std::fs::read_to_string(path).map_err(|_| ())?;
    let mut updated = String::new();
    let mut index = 0;
    let mut save_next_label: Option<String> = None;
    for line in contents.lines() {
        if let Some(label) = save_next_label.take() {
            updated.push_str("    ");
            updated.push_str(&label);
            updated.push('\n');
            if !line.starts_with("    #") && !line.starts_with("    ^") {
                continue;
            }
        }
        if !line.starts_with("https") && !line.starts_with("-") && !line.starts_with("+") {
            updated.push_str(line);
            updated.push('\n');
            continue;
        }
        let rest = line
            .strip_prefix("-")
            .or_else(|| line.strip_prefix("+"))
            .unwrap_or(line)
            .to_owned();
        assert!(rest.starts_with(&commits[index].url));
        match commits[index].state {
            State::Ignored => updated.push('-'),
            State::Accepted => updated.push('+'),
            State::Untriaged => {}
        }
        updated.push_str(&rest);
        updated.push('\n');
        save_next_label = if !commits[index].label.is_empty() {
            Some(commits[index].label.clone())
        } else {
            None
        };
        index += 1;
    }

    let mut buffer = File::create(path).map_err(|_| ())?;
    buffer.write_all(updated.as_bytes()).map_err(|_| ())?;
    Ok(())
}

pub fn parse_from_file(path: &Path) -> Result<Vec<Commit>, ()> {
    let contents = std::fs::read_to_string(path).map_err(|_| ())?;
    Ok(parse_from_str(&contents))
}

fn parse_from_str(contents: &str) -> Vec<Commit> {
    let mut commits = vec![];
    let mut commit = Commit::default();
    let mut current_date = String::new();
    for line in contents.lines() {
        if line.starts_with(">>>") {
            current_date = line.strip_prefix(">>> ").unwrap().to_owned();
            continue;
        }
        if let Some(rest) = line.strip_prefix("    ^ ") {
            if let Some(rest) = rest.strip_prefix("commit ") {
                commit.hash = rest.to_owned();
            }
            commit.hints.push(rest.to_owned());
            continue;
        }
        if let Some(rest) = line.strip_prefix("    # ") {
            commit.body.push(rest.to_owned());
            continue;
        }
        if let Some(rest) = line.strip_prefix("    ") {
            commit.label = rest.to_owned();
            continue;
        }
        if commit != Commit::default() {
            commits.push(commit);
            commit = Commit::default();
        }
        commit.date = current_date.clone();
        let line_rest;
        if let Some(rest) = line.strip_prefix("-") {
            commit.state = State::Ignored;
            line_rest = rest;
        } else if let Some(rest) = line.strip_prefix("+") {
            commit.state = State::Accepted;
            line_rest = rest;
        } else {
            line_rest = line;
        }
        let mut parts = line_rest.split("\t");
        commit.url = parts.next().unwrap().to_owned();
        let author_info = parts.next().unwrap();
        let author_info = author_info
            .strip_prefix("(")
            .unwrap()
            .strip_suffix(")")
            .unwrap();
        let mut author_info = author_info
            .split(",")
            .map(|part| part.trim().to_owned())
            .collect::<Vec<_>>();
        author_info.pop();
        commit.authors = author_info;
        if commit.authors.contains(&"@dependabot[bot]".to_owned())
            || commit.authors.contains(&"@servo-wpt-sync".to_owned())
        {
            commit.state = State::Ignored;
        }
        commit.title = parts.next().unwrap().to_owned();
    }
    commits.push(commit);
    commits
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let contents = r#">>> 2026-01-01T06:05:47Z
+https://github.com/servo/servo/pull/41604	(@kkoyung, #41604)	script: Implement export key operation of ML-KEM (#41604)
    dom; web crypto
    ^ commit c7cd8fcef8270718ae755f9f8f460247cb9f3b5b
    # Continue on adding ML-KEM support to WebCrypto API.  Specification:
    # https://wicg.github.io/webcrypto-modern-algos/#ml-kem
    # This patch implements export key operation of ML-KEM, with `ml-kem` crate.
    # Testing: Pass some WPT tests that were expected to fail.  Fixes: Part of #41473
-https://github.com/servo/servo/pull/41198	(@Narfinger, #41198)	Base: Rename IpcSharedMemory to GenericSharedMemory (#41198)
    ^ commit 15aa6ee8c037526ee3ec69eb761521d4ddbc2671
    ^ /!\ contains changes to WPT expectations! it probably affects the web platform
    # In the future, servo components should depend on the generic channels in base instead of IpcChannels to correctly
    # optimize for multiprocess vs non-multiprocess mode.  This reexports IpcSharedMemory as GenericSharedMemory in
    # GenericChannel and changes all dependencies on it.
    # Currently this is only a type/name change and does not change functionality.  But in the future we would want want to
    # use non-ipc things for the data.
    # Signed-off-by: Narfinger
    # Testing: This changes types and does not need testing."#;
        let commits = parse_from_str(contents);
        let expected = vec![
            Commit {
                url: "https://github.com/servo/servo/pull/41604".to_owned(),
                authors: vec!["@kkoyung".to_owned()],
                title: "script: Implement export key operation of ML-KEM (#41604)".to_owned(),
                hash: "c7cd8fcef8270718ae755f9f8f460247cb9f3b5b".to_owned(),
                hints: vec![
                    "commit c7cd8fcef8270718ae755f9f8f460247cb9f3b5b".to_owned(),
                ],
                body: vec![
                    "Continue on adding ML-KEM support to WebCrypto API.  Specification:".to_owned(),
                    "https://wicg.github.io/webcrypto-modern-algos/#ml-kem".to_owned(),
                    "This patch implements export key operation of ML-KEM, with `ml-kem` crate.".to_owned(),
                    "Testing: Pass some WPT tests that were expected to fail.  Fixes: Part of #41473".to_owned(),
                ],
                date: "2026-01-01T06:05:47Z".to_owned(),
                label: "dom; web crypto".to_owned(),
                state: State::Accepted,
            },
            Commit {
                url: "https://github.com/servo/servo/pull/41198".to_owned(),
                authors: vec!["@Narfinger".to_owned()],
                title: "Base: Rename IpcSharedMemory to GenericSharedMemory (#41198)".to_owned(),
                hash: "15aa6ee8c037526ee3ec69eb761521d4ddbc2671".to_owned(),
                hints: vec![
                    "commit 15aa6ee8c037526ee3ec69eb761521d4ddbc2671".to_owned(),
                    r"/!\ contains changes to WPT expectations! it probably affects the web platform".to_owned(),
                ],
                body: vec![
                    "In the future, servo components should depend on the generic channels in base instead of IpcChannels to correctly".to_owned(),
                    "optimize for multiprocess vs non-multiprocess mode.  This reexports IpcSharedMemory as GenericSharedMemory in".to_owned(),
                    "GenericChannel and changes all dependencies on it.".to_owned(),
                    "Currently this is only a type/name change and does not change functionality.  But in the future we would want want to".to_owned(),
                    "use non-ipc things for the data.".to_owned(),
                    "Signed-off-by: Narfinger".to_owned(),
                    "Testing: This changes types and does not need testing.".to_owned(),
                ],
                date: "2026-01-01T06:05:47Z".to_owned(),
                label: String::new(),
                state: State::Ignored,
            }
        ];
        assert_eq!(commits, expected);
    }
}
