use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};

use encoding_rs::WINDOWS_1252;
use serde::Serialize;

use crate::{
    ARGS,
    commit::{Commit, State},
};

#[derive(Debug, Default, Serialize)]
pub struct WordCloud {
    words: Vec<(String, Vec<WordCloudEntry>)>,
}

#[derive(Debug, Serialize)]
pub struct WordCloudEntry {
    hash_number: String,
    title: String,
}

pub fn compute_word_cloud(commits: &[Commit]) -> Result<WordCloud, &'static str> {
    static DICTIONARY: LazyLock<Option<BTreeSet<String>>> = LazyLock::new(|| {
        let file = std::fs::read(&*ARGS.dictionary_path).ok()?;
        // according to file(1), scowl-2020.12.07 /share/dict/words.txt is
        // ISO-8859 text, and ~300 words rely on this. decode it as
        // Windows-1252, which is a superset of ISO 8859-1.
        let (file, _actual_encoding, _malformed) = WINDOWS_1252.decode(&file);
        Some(file.split("\n").map(|word| word.to_owned()).collect())
    });
    let dictionary = DICTIONARY
        .as_ref()
        .ok_or("failed to read file at --dictionary-path")?;
    let mut result = WordCloud::default();
    #[derive(Default)]
    struct WordInfo {
        commits: Vec<Commit>,
    }
    let mut frequency: BTreeMap<String, WordInfo> = BTreeMap::default();
    for commit in commits {
        if !commit.label.is_empty() || commit.state != State::Untriaged {
            continue;
        }
        let words = commit
            .word_cloud_text()
            .split_ascii_whitespace()
            .map(|word| {
                let word = word.to_ascii_lowercase();
                let mut word = &*word;
                while let Some(stripped_word) = word.strip_prefix(['[', '(', '\'', '`']) {
                    word = stripped_word;
                }
                while let Some(stripped_word) =
                    word.strip_suffix([']', ')', ':', ',', '.', '\'', '`'])
                {
                    word = stripped_word;
                }
                word.to_owned()
            })
            .collect::<Vec<_>>();
        for ngram_len in 3..6 {
            for ngram in words.windows(ngram_len).collect::<BTreeSet<_>>() {
                frequency
                    .entry(ngram.join(" "))
                    .or_default()
                    .commits
                    .push(commit.clone());
            }
        }
        for word in words.into_iter().collect::<BTreeSet<_>>() {
            if word.contains(|c: char| c.is_alphanumeric()) && !dictionary.contains(&*word) {
                frequency
                    .entry(word.to_owned())
                    .or_default()
                    .commits
                    .push(commit.clone());
            }
        }
    }
    let mut frequency = frequency.into_iter().collect::<Vec<_>>();
    frequency.sort_by_key(|(_word, info)| info.commits.len());
    for (word, info) in frequency.into_iter().rev() {
        let count = info.commits.len();
        if count > 2 {
            let mut entries = vec![];
            for commit in info.commits {
                entries.push(WordCloudEntry {
                    hash_number: commit.hash_number,
                    title: commit.title,
                });
            }
            result.words.push((word, entries));
        }
    }
    Ok(result)
}
