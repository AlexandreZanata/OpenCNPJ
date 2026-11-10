//! Vocabulary audit: the words of evaluation questions that no catalogue entity covers.
//!
//! The interpreter refuses to execute a question that holds a word it does not know
//! (`opencnpj_nlu::lexicon`). This audit lists, for every such word of the evaluation sets,
//! how many executed gold questions use it, so the known vocabulary can be reviewed against
//! the corpus instead of guessed.

use crate::{
    eval,
    questions::{self, Assets, DataArgs},
};
use anyhow::Result;
use opencnpj_nlu::{context::Conversation, normalize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(clap::Args)]
pub struct VocabArgs {
    #[command(flatten)]
    pub data: DataArgs,
    /// Evaluation sets to read; repeat for several.
    #[arg(long, required = true)]
    pub set: Vec<PathBuf>,
    /// TSV: word, executed gold questions using it, other questions using it, known, example.
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Default)]
struct Usage {
    executed: u64,
    other: u64,
    example: String,
}

pub fn run(args: VocabArgs) -> Result<()> {
    let assets = Assets::open(&args.data)?;
    let mut usage: BTreeMap<String, Usage> = BTreeMap::new();
    for set in &args.set {
        let (mut state, mut session) = (Conversation::default(), String::new());
        for item in eval::load_items(set)? {
            if questions::session_of(&item) != session {
                (state, session) = (Conversation::default(), questions::session_of(&item));
            }
            // Keep the conversation in step with the set, as the evaluation does.
            let _ = assets.interpreter.interpret(&item.text, &mut state);
            let executed = matches!(item.gold.decision.as_str(), "execute" | "execute_with_note");
            let words = normalize::words(&item.text);
            let entities = assets.interpreter.entities(&item.text);
            for (at, word) in words.iter().enumerate() {
                if entities.iter().any(|(start, end, _)| *start <= at && at < *end) {
                    continue;
                }
                let entry = usage.entry(word.clone()).or_default();
                if executed {
                    entry.executed += 1;
                } else {
                    entry.other += 1;
                }
                if entry.example.is_empty() || (executed && entry.executed == 1) {
                    entry.example = item.text.clone();
                }
            }
        }
    }
    let mut rows: Vec<(&String, &Usage)> = usage.iter().collect();
    rows.sort_by(|a, b| b.1.executed.cmp(&a.1.executed).then_with(|| a.0.cmp(b.0)));
    let mut out = BufWriter::new(File::create(&args.out)?);
    writeln!(out, "word\texecuted\tother\tknown\texample")?;
    let mut unknown = 0u64;
    for (word, entry) in &rows {
        let known = assets.interpreter.knows(word);
        if !known && entry.executed > 0 {
            unknown += 1;
        }
        writeln!(out, "{word}\t{}\t{}\t{}\t{}", entry.executed, entry.other, u8::from(known), entry.example.replace(['\t', '\n'], " "))?;
    }
    out.flush()?;
    println!("{} distinct words outside entities; {unknown} used by executed gold questions are not in the known vocabulary", rows.len());
    Ok(())
}
