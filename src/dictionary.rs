//! High-performance dictionary.

use anyhow::{format_err, Context, Result};
use fst::{IntoStreamer, Map, MapBuilder, Streamer};
use memmap2::Mmap;
use once_cell::sync::Lazy;
use regex::Regex;
use regex_automata::dense;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader, BufWriter},
    path::Path,
    str::from_utf8,
};

use crate::probability::{Dist, Prob};

/// A high-performance dictionary of English-language words.
pub struct Dictionary {
    words: Map<Mmap>,
}

impl Dictionary {
    /// Build a new dictionary and write it to disk.
    pub fn build(in_words_path: &Path, out_dict_path: &Path) -> Result<()> {
        // Compile our regex.
        static COUNT_RE: Lazy<Regex> = Lazy::new(|| {
            // We permit leading whitespace for compatibility with `uniq -c`.
            Regex::new("^\\s*([0-9]+)\\s+(.+)\r?\n?$")
                .expect("invalid regex in source")
        });

        // Load our count information.
        let mut total_count: u64 = 0;
        let mut counts = BTreeMap::<String, u64>::new();
        let in_words_file = File::open(in_words_path)
            .with_context(|| format!("could not open {}", in_words_path.display()))?;
        let in_words_rdr = BufReader::new(in_words_file);
        for line in in_words_rdr.lines() {
            let line = line.with_context(|| {
                format!("could not read from {}", in_words_path.display())
            })?;
            if let Some(cap) = COUNT_RE.captures(&line) {
                let count = cap[1]
                    .parse::<u64>()
                    .with_context(|| format!("could not parse count {:?}", &cap[1]))?;
                let word = cap[2].to_ascii_lowercase();
                if counts.insert(word, count).is_some() {
                    return Err(format_err!("duplicate word {:?}", &cap[2]));
                }
                total_count = total_count.checked_add(count).ok_or_else(|| {
                    format_err!("total word count is too large for u64")
                })?;
            } else {
                return Err(format_err!(
                    "expected \"count\\s+word\", found {:?}",
                    line
                ));
            }
        }

        // Open our output file.
        let out_dict_file = File::create(out_dict_path).with_context(|| {
            format!("could not create {}", out_dict_path.display())
        })?;
        let out_dict_wtr = BufWriter::new(out_dict_file);
        let mut builder = MapBuilder::new(out_dict_wtr).with_context(|| {
            format!("could not create dictionary {}", out_dict_path.display())
        })?;

        // Add words to our dictionary.
        for (word, count) in counts {
            let prob = Prob::from_fraction(count, total_count);
            builder
                .insert(word.as_bytes(), prob.to_bits())
                .with_context(|| {
                    format!("could not write to {}", out_dict_path.display())
                })?;
        }

        // Finish writing our dictionary to disk.
        builder.finish().with_context(|| {
            format!("could not write to {}", out_dict_path.display())
        })?;
        Ok(())
    }

    pub fn load(dict_path: &Path) -> Result<Dictionary> {
        // We need to use `unsafe` because bad things can happen if someone
        // modifies the file while we're using it.
        let dict_file = File::open(dict_path)
            .with_context(|| format!("error opening {}", dict_path.display()))?;
        let mapped = unsafe { Mmap::map(&dict_file) }
            .with_context(|| format!("error mapping {}", dict_path.display()))?;
        let words = Map::new(mapped).with_context(|| {
            format!("error initializing dictionary {}", dict_path.display())
        })?;
        Ok(Dictionary { words })
    }

    pub fn find_matches(&self, regex: &str) -> Result<Dist<String>> {
        let dfa = dense::Builder::new().anchored(true).build(&regex)?;
        let mut stream = self.words.search(&dfa).into_stream();
        let mut events = vec![];
        while let Some((word_bytes, prob_bits)) = stream.next() {
            let prob = Prob::from_bits(prob_bits);
            let word = from_utf8(word_bytes)
                .context("dict contains invalid UTF-8")?
                .to_owned();
            events.push((prob, word));
        }
        let mut dist = Dist::from_vec(events);
        dist.sort_by_probability();
        Ok(dist)
    }
}
