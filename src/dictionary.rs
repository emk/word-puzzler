//! High-performance dictionary.

use anyhow::{format_err, Context, Result};
use fst::{IntoStreamer, Map, MapBuilder};
use memmap2::Mmap;
use once_cell::sync::Lazy;
use regex::Regex;
use regex_automata::dense;
use std::{
    cmp::min,
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, BufWriter},
    path::Path,
};

/// A high-performance dictionary of English-language words.
pub struct Dictionary {
    words: Map<Mmap>,
}

impl Dictionary {
    /// Build a new dictionary and write it to disk.
    pub fn build(
        in_words_path: &Path,
        in_count_path: &Path,
        out_dict_path: &Path,
    ) -> Result<()> {
        // Compile our regex.
        static COUNT_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new("^([^ \t]+)[ \t]+([0-9]+)\r?\n?$")
                .expect("invalid regex in source")
        });

        // Load our count information.
        let mut counts = HashMap::<String, u64>::new();
        let mut total_count: u64 = 0;
        let mut smallest_count: u64 = u64::MAX;
        let in_count_file = File::open(in_count_path)
            .with_context(|| format!("could not open {}", in_count_path.display()))?;
        let in_count_rdr = BufReader::new(in_count_file);
        for line in in_count_rdr.lines() {
            let line = line.with_context(|| {
                format!("could not read from {}", in_count_path.display())
            })?;
            if let Some(cap) = COUNT_RE.captures(&line) {
                let word = cap[1].to_ascii_lowercase();
                let count = cap[2]
                    .parse::<u64>()
                    .with_context(|| format!("could not parse count {:?}", &cap[2]))?;
                if counts.insert(word, count).is_some() {
                    return Err(format_err!("duplicate word {:?}", &cap[0]));
                }
                total_count = total_count.checked_add(count).ok_or_else(|| {
                    format_err!(
                        "word count in {} overflows u64",
                        in_count_path.display()
                    )
                })?;
                smallest_count = min(count, smallest_count);
            } else {
                return Err(format_err!(
                    "expected \"word[ \t]+count\", found {:?}",
                    line
                ));
            }
        }
        if smallest_count == u64::MAX {
            smallest_count = 1;
        }

        // Open our input words file.
        let in_words_file = File::open(in_words_path)
            .with_context(|| format!("could not open {}", in_words_path.display()))?;
        let in_words_rdr = BufReader::new(in_words_file);

        // Open our output file.
        let out_dict_file = File::create(out_dict_path).with_context(|| {
            format!("could not create {}", out_dict_path.display())
        })?;
        let out_dict_wtr = BufWriter::new(out_dict_file);
        let mut builder = MapBuilder::new(out_dict_wtr).with_context(|| {
            format!("could not create dictionary {}", out_dict_path.display())
        })?;

        // Add words to our dictionary.
        for line in in_words_rdr.lines() {
            let line = line
                .with_context(|| {
                    format!("could not read from {}", in_words_path.display())
                })?
                .trim_end_matches(|c| c == '\r' || c == '\n')
                .to_ascii_lowercase();
            let prob = counts.get(&line).map_or_else(|| smallest_count, |c| *c) as f64
                / total_count as f64;
            let bytes = line.as_bytes();
            builder.insert(bytes, prob.to_bits()).with_context(|| {
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

    pub fn find_matches(&self, regex: &str) -> Result<Vec<(String, f64)>> {
        let regex_lower = regex.to_ascii_lowercase();
        let dfa = dense::Builder::new().anchored(true).build(&regex_lower)?;
        let matches = self
            .words
            .search(&dfa)
            .into_stream()
            .into_str_vec()?
            .into_iter()
            .map(|(w, p)| (w, f64::from_bits(p)))
            .collect::<Vec<_>>();
        Ok(matches)
    }
}
