//! Builds BaramOS's compact Simplified Chinese candidate index from the
//! Apache-2.0 AOSP PinyinIME `rawdict_utf16_65105_freq.txt` source dictionary.
//!
//! Usage:
//!   rustc tools/generate_pinyin_dictionary.rs -O -o /tmp/generate_pinyin_dictionary
//!   /tmp/generate_pinyin_dictionary \
//!     /path/to/PinyinIME/jni/data/rawdict_utf16_65105_freq.txt \
//!     crates/baram-boot/src/pinyin_dictionary.tsv

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;

const MAX_CANDIDATES: usize = 8;

fn decode_utf16le(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes);
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).expect("AOSP dictionary must be valid UTF-16LE")
}

fn main() {
    let mut args = env::args().skip(1);
    let source_path = args.next().expect("raw AOSP dictionary path is required");
    let output_path = args.next().expect("output TSV path is required");
    assert!(args.next().is_none(), "unexpected argument");

    // Records are Hanzi, frequency, GBK flag, then one spelling per Hanzi.
    // Flag 0 is the standard Simplified-Chinese dictionary set.
    let source = decode_utf16le(&fs::read(&source_path).expect("read AOSP dictionary"));
    let mut entries: BTreeMap<String, Vec<(u64, String)>> = BTreeMap::new();
    for line in source.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let Some((word, rest)) = fields.split_first() else { continue };
        if rest.len() < 3 || rest[1] != "0" || word.chars().count() != rest.len() - 2 {
            continue;
        }
        let Some(frequency) = rest[0].parse::<f64>().ok() else { continue };
        let key = rest[2..].concat().to_ascii_lowercase();
        if key.is_empty() || !key.bytes().all(|byte| byte.is_ascii_lowercase()) {
            continue;
        }
        entries.entry(key).or_default().push(((frequency * 1_000.0) as u64, (*word).to_owned()));
    }

    let mut output = String::from(
        "# Generated from AOSP PinyinIME rawdict_utf16_65105_freq.txt (Apache-2.0). Do not edit manually.\n",
    );
    for (key, mut candidates) in entries {
        candidates.sort_unstable_by(|left, right| right.cmp(left));
        let mut seen = HashSet::new();
        output.push_str(&key);
        for (_, candidate) in candidates.into_iter().filter(|(_, word)| seen.insert(word.clone())).take(MAX_CANDIDATES) {
            output.push('\t');
            output.push_str(&candidate);
        }
        output.push('\n');
    }
    fs::write(&output_path, output).expect("write Pinyin candidate index");
}
