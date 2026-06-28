//! Toy eval: best-effort LEXICAL search over our docs with plain tantivy (BM25).
//! No embeddings, no synonym/alias dictionaries, no query rewriting cheats.
//!
//! Honest IR techniques only:
//!   - per-heading chunks (path/title/body)
//!   - English stemming on title/body (recall for plurals/inflections)
//!   - a char-trigram field over title+body (typo / morphology tolerance — it is a
//!     tokenization scheme, not a synonym table)
//!   - edismax-style query assembly, all OR-combined, BM25 scores the rest:
//!       * exact phrase boost on title (4x) and body (2x)
//!       * per-term stemmed match: title (3x) + body (1x)
//!       * analyzer-consistent fuzzy (Levenshtein 1) on body (0.3x) — query token is
//!         stemmed the SAME way as the index so it actually matches
//!       * trigram overlap on the ngram field (0.15x) — rescues typos with no exact term
//!
//! Usage:
//!   tantivy-eval [docs.jsonl]            # run the built-in query battery
//!   tantivy-eval [docs.jsonl] -q "..."   # one ad-hoc query (baseline + best)

use std::env;
use std::fs;

use tantivy::collector::TopDocs;
use tantivy::query::{
    BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, QueryParser, TermQuery,
};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING,
};
use tantivy::tokenizer::{LowerCaser, NgramTokenizer, TextAnalyzer};
use tantivy::{doc, Index, TantivyDocument, Term};

struct Fields {
    path: Field,
    title: Field,
    body: Field,
    ngram: Field,
}

fn build_index(jsonl: &str) -> (Index, Fields) {
    let mut sb = Schema::builder();
    let path = sb.add_text_field("path", STRING | STORED);

    let stemmed = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("en_stem")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    let title = sb.add_text_field("title", stemmed.clone());
    let body = sb.add_text_field("body", stemmed);

    // char trigrams (lowercased) — typo/substring tolerance without synonyms
    let ngram_opts = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("char3")
            .set_index_option(IndexRecordOption::WithFreqs),
    );
    let ngram = sb.add_text_field("ngram", ngram_opts);

    let schema = sb.build();
    let index = Index::create_in_ram(schema);

    // Register the trigram analyzer BEFORE writing.
    let trigram = TextAnalyzer::builder(NgramTokenizer::new(3, 3, false).unwrap())
        .filter(LowerCaser)
        .build();
    index.tokenizers().register("char3", trigram);

    let fields = Fields {
        path,
        title,
        body,
        ngram,
    };
    let mut writer = index.writer(50_000_000).expect("writer");

    let mut n = 0usize;
    for line in jsonl.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let p = v.get("path").and_then(|x| x.as_str()).unwrap_or("");
        let t = v.get("title").and_then(|x| x.as_str()).unwrap_or("");
        let b = v.get("body").and_then(|x| x.as_str()).unwrap_or("");
        let tb = format!("{t} {b}");
        writer
            .add_document(doc!(
                fields.path => p,
                fields.title => t,
                fields.body => b,
                fields.ngram => tb,
            ))
            .expect("add");
        n += 1;
    }
    writer.commit().expect("commit");
    eprintln!("indexed {n} chunks");
    (index, fields)
}

/// Tokenize `text` with a registered analyzer, returning the produced terms.
fn analyze(index: &Index, tokenizer: &str, text: &str) -> Vec<String> {
    let mut a = index.tokenizers().get(tokenizer).expect("tokenizer");
    let mut out = Vec::new();
    let mut ts = a.token_stream(text);
    while ts.advance() {
        out.push(ts.token().text.clone());
    }
    out
}

fn boosted(q: Box<dyn Query>, b: f32) -> Box<dyn Query> {
    Box::new(BoostQuery::new(q, b))
}

fn term_q(field: Field, text: &str) -> Box<dyn Query> {
    Box::new(TermQuery::new(
        Term::from_field_text(field, text),
        IndexRecordOption::WithFreqs,
    ))
}

/// edismax-style lexical query. All clauses are SHOULD; BM25 ranks the union.
fn best_query(index: &Index, f: &Fields, query: &str) -> Box<dyn Query> {
    let terms = analyze(index, "en_stem", query);
    let mut should: Vec<(Occur, Box<dyn Query>)> = Vec::new();

    // exact phrase boost (only fires when the phrase actually occurs)
    if terms.len() >= 2 {
        let title_terms: Vec<Term> = terms
            .iter()
            .map(|t| Term::from_field_text(f.title, t))
            .collect();
        should.push((
            Occur::Should,
            boosted(Box::new(PhraseQuery::new(title_terms)), 4.0),
        ));
        let body_terms: Vec<Term> = terms
            .iter()
            .map(|t| Term::from_field_text(f.body, t))
            .collect();
        should.push((
            Occur::Should,
            boosted(Box::new(PhraseQuery::new(body_terms)), 2.0),
        ));
    }

    // per-term: title (3x) + body (1x) + analyzer-consistent fuzzy on body (0.3x)
    for t in &terms {
        should.push((Occur::Should, boosted(term_q(f.title, t), 3.0)));
        should.push((Occur::Should, boosted(term_q(f.body, t), 1.0)));
        let fq = FuzzyTermQuery::new(Term::from_field_text(f.body, t), 1, true);
        should.push((Occur::Should, boosted(Box::new(fq), 0.3)));
    }

    // trigram overlap (low boost) — rescues typos / unseen inflections
    for tri in analyze(index, "char3", query) {
        should.push((Occur::Should, boosted(term_q(f.ngram, &tri), 0.15)));
    }

    Box::new(BooleanQuery::new(should))
}

fn show(index: &Index, f: &Fields, query: &dyn Query, k: usize) {
    let searcher = index.reader().expect("reader").searcher();
    let top = searcher
        .search(query, &TopDocs::with_limit(k))
        .expect("search");
    if top.is_empty() {
        println!("    (no hits)");
        return;
    }
    for (rank, (score, addr)) in top.iter().enumerate() {
        let d: TantivyDocument = searcher.doc(*addr).expect("doc");
        let path = d.get_first(f.path).and_then(|v| v.as_str()).unwrap_or("?");
        let title = d.get_first(f.title).and_then(|v| v.as_str()).unwrap_or("?");
        println!("    {}. [{:.3}] {}  ::  {}", rank + 1, score, path, title);
    }
}

fn run(index: &Index, f: &Fields, query: &str, k: usize) {
    println!("\n=== query: {query:?} ===");

    let mut qp = QueryParser::for_index(index, vec![f.title, f.body]);
    qp.set_field_boost(f.title, 3.0);
    println!("  [baseline: parser OR, title^3]");
    match qp.parse_query(query) {
        Ok(q) => show(index, f, &*q, k),
        Err(e) => println!("    parse error: {e}"),
    }

    println!("  [best: phrase+term+fuzzy+trigram]");
    show(index, f, &*best_query(index, f, query), k);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut jsonl_path = "docs.jsonl".to_string();
    let mut adhoc: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-q" => {
                adhoc = args.get(i + 1).cloned();
                i += 2;
            }
            other => {
                jsonl_path = other.to_string();
                i += 1;
            }
        }
    }

    let jsonl = fs::read_to_string(&jsonl_path).expect("read docs.jsonl");
    let (index, f) = build_index(&jsonl);
    let k = 5;

    if let Some(q) = adhoc {
        run(&index, &f, &q, k);
        return;
    }

    let battery = [
        "how to install qmdc",
        "semantic search",
        "broken link validation",
        "rename an object and update references",
        "query objects with sql",
        "list mcp tools",
        "namespace reference syntax",
        "vscode extension features",
        "typed edges",
        "ignore files from the workspace",
        "render docs to a website", // vocabulary mismatch (docs say "SSG"/"mkdocs")
        "find all references to an object",
        "referense resolusion", // typos: reference resolution
    ];

    for q in battery {
        run(&index, &f, q, k);
    }
}
