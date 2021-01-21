use anyhow::Result;
use log::debug;
use ordered_float::OrderedFloat;
use std::{cmp::Ord, path::PathBuf};
use structopt::StructOpt;

mod dictionary;

use crate::dictionary::Dictionary;

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
    MakeDictionary {
        /// A list of valid words, one per line.
        in_words_path: PathBuf,
        /// A list of words and their count, separated by spaces, one per line.
        in_count_path: PathBuf,
        /// The output dictionary.
        out_dict_path: PathBuf,
    },

    /// Look up words matching a regular expression.
    Search {
        /// The dictionary to search.
        dict_path: PathBuf,
        /// A regex describing the word (automatically anchored on both ends).
        regex: String,
        /// Show the probability of each word.
        #[structopt(short = "p")]
        show_probability: bool,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let opt = Opt::from_args();
    debug!("options: {:?}", opt);

    match &opt.cmd {
        Command::MakeDictionary {
            in_words_path,
            in_count_path,
            out_dict_path,
        } => {
            Dictionary::build(in_words_path, in_count_path, out_dict_path)?;
        }
        Command::Search {
            dict_path,
            regex,
            show_probability,
        } => {
            let dict = Dictionary::load(dict_path)?;
            let mut matches = dict.find_matches(regex)?;
            matches.sort_by(|(_, prob1), (_, prob2)| {
                Ord::cmp(&OrderedFloat(*prob2), &OrderedFloat(*prob1))
            });
            for m in matches {
                if *show_probability {
                    println!("{} {:.7}", m.0, m.1);
                } else {
                    println!("{}", m.0);
                }
            }
        }
    }
    Ok(())
}
