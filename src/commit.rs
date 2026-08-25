use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum State {
    #[default]
    Untriaged,
    Ignored,
    Accepted,
    Done,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Commit {
    pub index_in_file: usize,
    pub url: String,
    pub authors: Vec<String>,
    pub hash_number: String,
    pub title: String,
    pub hash: String,
    pub hints: Vec<String>,
    pub body: Vec<String>,
    pub date: String,
    pub state: State,
    pub label: String,
}

impl Commit {
    pub fn number(&self) -> &str {
        self.hash_number
            .strip_prefix("#")
            .expect("guaranteed by format")
    }

    pub fn tags(&self) -> &str {
        if let Some((tags, _notes)) = self.label.split_once(";") {
            return tags;
        }
        &self.label
    }

    pub fn notes(&self) -> Option<&str> {
        if let Some((_tags, notes)) = self.label.split_once(";") {
            return Some(notes);
        }
        None
    }

    pub fn filter_text(&self) -> String {
        let mut lines = vec![self.title.clone(), self.authors.join(", ")];
        lines.extend_from_slice(&self.hints);
        lines.extend_from_slice(&self.body);
        lines.join("\n")
    }
    pub fn word_cloud_text(&self) -> String {
        let mut lines = vec![self.title.clone()];
        lines.extend_from_slice(&self.body);
        lines.join("\n")
    }
}

pub fn write_to_file(
    commits: &[Commit],
    path: &Path,
    other_commits: Option<&[Commit]>,
) -> Result<(), ()> {
    let mut commits = commits.to_owned();
    commits.sort_by_key(|commit| commit.index_in_file);

    let mut result = String::new();
    let mut last_date = None;
    for (commit, other_commit) in commits.iter().zip(other_commits.unwrap_or(&commits)) {
        if last_date != Some(&commit.date) {
            writeln!(result, ">>> {}", commit.date).unwrap();
            last_date = Some(&commit.date);
        }
        match commit.state {
            State::Untriaged => {}
            State::Ignored => write!(result, "-").unwrap(),
            State::Accepted => write!(result, "+").unwrap(),
            State::Done => write!(result, ".").unwrap(),
        }
        writeln!(
            result,
            "{}\t({}, {})\t{}",
            other_commit.url,
            other_commit.authors.join(", "),
            other_commit.hash_number,
            other_commit.title
        )
        .unwrap();
        if !commit.label.is_empty() {
            writeln!(result, "    {}", commit.label).unwrap();
        }
        for line in other_commit.hints.iter() {
            writeln!(result, "    ^ {line}").unwrap();
        }
        for line in other_commit.body.iter() {
            writeln!(result, "    # {line}").unwrap();
        }
    }

    // write out `commits.txt` in as few write calls as possible.
    // if we write it line by line, and the file is in the servo.org checkout, any local eleventy
    // server will crash as follows, even if we add `commits.txt` to `.eleventyignore`:
    // FATAL ERROR: Reached heap limit Allocation failed - JavaScript heap out of memory
    std::fs::write(path, &result).map_err(|_| ())?;

    Ok(())
}

pub fn parse_from_file(path: &Path) -> Result<Vec<Commit>, ()> {
    let contents = std::fs::read_to_string(path).map_err(|_| ())?;
    Ok(parse_from_str(&contents))
}

fn parse_from_str(contents: &str) -> Vec<Commit> {
    let mut commits = vec![];
    let mut commit = Commit::default();
    let mut started_first_commit = false;
    let mut current_date = String::new();
    let mut has_bad_input = false;
    let state_prefixes = ['-', '+', '.'];
    for (line_index, line) in contents.lines().enumerate() {
        let line_number = line_index + 1;
        if line.starts_with(">>>") {
            current_date = line.strip_prefix(">>> ").unwrap().to_owned();
            continue;
        }
        if started_first_commit {
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
            if line.starts_with("https://") || line.starts_with(state_prefixes) {
                commits.push(commit);
                commit = Commit::default();
                commit.index_in_file = commits.len();
            } else {
                eprintln!("bad input on line {line_number}: {line:?}");
                has_bad_input = true;
                continue;
            }
        }
        started_first_commit = true;
        commit.date = current_date.clone();
        let line_rest;
        if let Some(rest) = line.strip_prefix(state_prefixes) {
            commit.state = match &line[0..1] {
                "-" => State::Ignored,
                "+" => State::Accepted,
                "." => State::Done,
                _ => unreachable!("guaranteed by strip_prefix() argument"),
            };
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
        commit.hash_number = author_info.pop().unwrap();
        commit.authors = author_info;
        if commit.authors.contains(&"@dependabot[bot]".to_owned())
            || commit.authors.contains(&"@servo-wpt-sync".to_owned())
        {
            commit.state = State::Ignored;
        }
        commit.title = parts.next().unwrap().to_owned();
    }
    commits.push(commit);
    if has_bad_input {
        panic!(
            "bad input! has the format changed? \
            if not, remove the lines above and try again"
        );
    }
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
                index_in_file: 0,
                url: "https://github.com/servo/servo/pull/41604".to_owned(),
                authors: vec!["@kkoyung".to_owned()],
                hash_number: "#41604".to_owned(),
                title: "script: Implement export key operation of ML-KEM (#41604)".to_owned(),
                hash: "c7cd8fcef8270718ae755f9f8f460247cb9f3b5b".to_owned(),
                hints: vec![
                    r"commit c7cd8fcef8270718ae755f9f8f460247cb9f3b5b".to_owned(),
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
                index_in_file: 1,
                url: "https://github.com/servo/servo/pull/41198".to_owned(),
                authors: vec!["@Narfinger".to_owned()],
                hash_number: "#41198".to_owned(),
                title: "Base: Rename IpcSharedMemory to GenericSharedMemory (#41198)".to_owned(),
                hash: "15aa6ee8c037526ee3ec69eb761521d4ddbc2671".to_owned(),
                hints: vec![
                    r"commit 15aa6ee8c037526ee3ec69eb761521d4ddbc2671".to_owned(),
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

    #[test]
    #[should_panic]
    fn test_bad_input() {
        let contents = r#">>> 2026-04-19T23:41:46Z
https://github.com/servo/servo/pull/44339   (@servo-wpt-sync, #44339)   Sync WPT with upstream (19-04-2026) (#44339)
    ^ commit 345fd4573f5a4d8cb3d0efe7a128ddfe6043db8b
warning: exhaustive rename detection was skipped due to too many files.
warning: you may want to set your diff.renameLimit variable to at least 53941 and retry the command.
warning: exhaustive rename detection was skipped due to too many files.
warning: you may want to set your diff.renameLimit variable to at least 53941 and retry the command.
warning: exhaustive rename detection was skipped due to too many files.
warning: you may want to set your diff.renameLimit variable to at least 53941 and retry the command.
warning: exhaustive rename detection was skipped due to too many files.
warning: you may want to set your diff.renameLimit variable to at least 53941 and retry the command.
warning: exhaustive rename detection was skipped due to too many files.
warning: you may want to set your diff.renameLimit variable to at least 53941 and retry the command.
warning: exhaustive rename detection was skipped due to too many files.
warning: you may want to set your diff.renameLimit variable to at least 53941 and retry the command.
    # Automated downstream sync of changes from upstream as of 19-04-2026
    # [no-wpt-sync]"#;
        parse_from_str(contents);
    }
}
