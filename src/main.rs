use anyhow::Result;
use itertools::Itertools;
use log::{debug, trace};
use std::{collections::BTreeSet, iter::Iterator, path::PathBuf};
use structopt::StructOpt;

mod dictionary;
mod probability;

use crate::dictionary::Dictionary;
use crate::probability::{Dist, Prob};

/// Command-line options.
#[derive(Debug, StructOpt)]
struct Opt {
    /// Subcommands.
    #[structopt(subcommand)]
    cmd: Command,
}

/// Our subcommands.
#[derive(Debug, StructOpt)]
enum Command {
    /// Create a new dictionary.
    #[structopt(name = "mkdict")]
    MakeDictionary(MakeDictionaryOpt),

    /// Look up words matching a regular expression.
    Search(SearchOpt),

    /// Permute letters or word fragments.
    Permute(PermuteOpt),
}

#[derive(Debug, StructOpt)]
struct MakeDictionaryOpt {
    /// A list of "\s*count\s+word" pairs, one per line.
    in_words_path: PathBuf,
    /// The output dictionary.
    out_dict_path: PathBuf,
}

#[derive(Debug, StructOpt)]
struct SearchOpt {
    /// The dictionary to search.
    dict_path: PathBuf,
    /// A regex describing the word (automatically anchored on both ends).
    regex: String,
}

#[derive(Debug, StructOpt)]
struct PermuteOpt {
    /// The dictionary to search.
    dict_path: PathBuf,
    /// Letters or fragments to permute. You can use "." as a placeholder for
    /// unknown letters.
    fragments: Vec<String>,
}

fn main() -> Result<()> {
    env_logger::init();
    let opt = Opt::from_args();
    debug!("options: {:?}", opt);

    match &opt.cmd {
        Command::MakeDictionary(mkdict_opt) => make_dictionary_cmd(mkdict_opt),
        Command::Search(search_opt) => search_cmd(search_opt),
        Command::Permute(permute_opt) => permute_cmd(permute_opt),
    }
}

fn make_dictionary_cmd(opt: &MakeDictionaryOpt) -> Result<()> {
    Dictionary::build(&opt.in_words_path, &opt.out_dict_path)?;
    Ok(())
}

fn search_cmd(opt: &SearchOpt) -> Result<()> {
    let dict = Dictionary::load(&opt.dict_path)?;
    let matches = dict.find_matches(&opt.regex)?;
    print!("{}", matches);
    Ok(())
}

fn permute_cmd(opt: &PermuteOpt) -> Result<()> {
    let dict = Dictionary::load(&opt.dict_path)?;
    let mut matches = vec![];
    let mut seen_candidates = BTreeSet::new();
    for permutation in opt
        .fragments
        .iter()
        .map(|s| &s[..])
        .permutations(opt.fragments.len())
    {
        let candidate = permutation.concat();
        if seen_candidates.contains(&candidate) {
            continue;
        }
        trace!("candidate: {}", candidate);
        seen_candidates.insert(candidate.clone());
        let mut so_far = vec![];
        break_into_words(&dict, &mut so_far, &candidate, &mut matches)?;
    }
    let mut matches = Dist::from_vec(matches);
    matches.sort_by_probability();
    print!("{}", matches);
    Ok(())
}

fn break_into_words(
    dict: &Dictionary,
    so_far: &mut Vec<(Prob, String)>,
    remaining_pattern: &str,
    matches: &mut Vec<(Prob, String)>,
) -> Result<()> {
    if remaining_pattern.len() == 0 {
        let mut prob = Prob::always();
        let mut words = String::new();
        for (p, w) in so_far {
            prob = prob * *p;
            if words.len() > 0 {
                words.push(' ');
            }
            words.push_str(w);
        }
        debug!("Found {} {}", prob, words);
        matches.push((prob, words));
    } else {
        for i in (1..=remaining_pattern.len()).rev() {
            let word_pat = &remaining_pattern[..i];
            let rest = &remaining_pattern[i..];

            let word_matches = dict.find_matches(&word_pat)?;
            for (p, w) in &word_matches {
                so_far.push((p, w.to_owned()));
                trace!("Trying {:?}", so_far);
                break_into_words(dict, so_far, rest, matches)?;
                so_far.pop();
            }
        }
    }
    Ok(())
}
