//! Builds BaramOS's compact kana-to-kanji lookup table from Mozc's OSS
//! `src/data/dictionary_oss/dictionary0*.txt` source files.
//!
//! Usage:
//!   rustc tools/generate_mozc_dictionary.rs -O -o /tmp/generate_mozc_dictionary
//!   /tmp/generate_mozc_dictionary /path/to/dictionary_oss crates/baram-boot/src/mozc_dictionary.tsv

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const MAX_COST: u32 = 5_000;
const MAX_CANDIDATES: usize = 3;

fn contains_kanji(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch as u32, 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff))
}

fn main() {
    let mut args = env::args().skip(1);
    let source_dir = args.next().expect("dictionary_oss directory is required");
    let output_path = args.next().expect("output TSV path is required");
    assert!(args.next().is_none(), "unexpected argument");

    let mut entries: BTreeMap<String, Vec<(u32, String)>> = BTreeMap::new();
    for index in 0..10 {
        let path = Path::new(&source_dir).join(format!("dictionary{index:02}.txt"));
        let file = File::open(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        for line in BufReader::new(file).lines() {
            let line = line.expect("dictionary must be valid UTF-8");
            let mut fields = line.split('\t');
            let Some(key) = fields.next() else { continue };
            let _left_id = fields.next();
            let _right_id = fields.next();
            let Some(cost) = fields.next().and_then(|cost| cost.parse::<u32>().ok()) else { continue };
            let Some(value) = fields.next() else { continue };
            if cost > MAX_COST || !contains_kanji(value) {
                continue;
            }
            entries.entry(key.to_owned()).or_default().push((cost, value.to_owned()));
        }
    }

    let mut output = String::from("# Generated from Google Mozc dictionary_oss. Do not edit manually.\n");
    for (key, mut candidates) in entries {
        candidates.sort_unstable();
        candidates.dedup_by(|left, right| left.1 == right.1);
        if candidates.is_empty() {
            continue;
        }
        output.push_str(&key);
        for (_, candidate) in candidates.into_iter().take(MAX_CANDIDATES) {
            output.push('\t');
            output.push_str(&candidate);
        }
        output.push('\n');
    }

    fs::write(&output_path, output).unwrap_or_else(|err| panic!("{}: {err}", output_path));
    let mut notice = File::create(format!("{output_path}.notice"))
        .expect("create generated dictionary notice");
    writeln!(notice, "Generated from Mozc dictionary_oss with MAX_COST={MAX_COST} and MAX_CANDIDATES={MAX_CANDIDATES}.")
        .expect("write generated dictionary notice");
}
