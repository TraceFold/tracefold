// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

mod atom;
mod extract;
mod gate;
mod manifest;
mod route;
mod serve;
mod store;

use clap::{CommandFactory, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::exit;

const EXIT_CODES: [(&str, &str, &str); 3] = [
    (
        "0",
        "answered",
        "the command ran and returned at least one row, or every gate it ran passed",
    ),
    (
        "1",
        "a gate counted a failure",
        "a gate found a real break: an orphan document, a duplicate id, a chain break, a stale index, a comment outside the header",
    ),
    (
        "2",
        "refused, or UNTESTABLE",
        "the question was malformed or could not be asked: an unknown filter value, a projection over its cap or budget, a cursor that is not a row id, an empty answer, an empty corpus, an unreadable manifest",
    ),
];

#[derive(Parser)]
#[command(
    name = "db",
    version,
    about = "compile a document corpus into a semantic index and answer questions about it"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "the DB directory holding db.toml; found upward from the working directory, or from DB_DIR, when it is not given"
    )]
    db: Option<String>,
    #[arg(
        long = "dump-commands",
        help = "print this command tree and the exit codes as json, for the README sync gate"
    )]
    dump_commands: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "read the source, build the semantic index, print the counts and the digests")]
    Compile,
    #[command(about = "record the atoms that changed as admission events in the journal, then compile")]
    Push,
    #[command(about = "run every source, index and query gate over this DB and print pass, fail and UNKNOWN")]
    Gate,
    #[command(about = "list the atoms of a projection")]
    Ls {
        #[arg(long, help = "keep only the atoms of one band")]
        band: Option<String>,
        #[arg(long, help = "keep only the atoms declared at one layer: L0, L1 or L2")]
        layer: Option<String>,
        #[arg(long, help = "keep only the atoms of documents with one role")]
        role: Option<String>,
        #[arg(long, help = "keep only the atoms written by one executor")]
        executor: Option<String>,
        #[arg(long, default_value_t = 0, help = "how much of each atom to render: 0 headline, 1 body, 2 body with provenance and relations")]
        lod: usize,
        #[arg(long, help = "the exact id printed at the end of the previous page, or begin")]
        cursor: Option<String>,
        #[arg(long, help = "print the wire json a face consumes instead of the text a person reads")]
        json: bool,
    },
    #[command(about = "print one atom named by its id or by its exact address")]
    Show {
        #[arg(help = "an atom id, an anchor, path#anchor, or band/path#anchor")]
        address: String,
        #[arg(long, default_value_t = 1, help = "how much of the atom to render: 0, 1 or 2")]
        lod: usize,
        #[arg(long, help = "print the wire json a face consumes instead of the text a person reads")]
        json: bool,
    },
    #[command(about = "search the full text index and return addresses and scores, never bodies")]
    Find {
        #[arg(help = "the text to look for")]
        needle: String,
        #[arg(long, help = "keep only the hits in one band")]
        band: Option<String>,
        #[arg(long, help = "keep only the hits at one layer")]
        layer: Option<String>,
        #[arg(long, default_value_t = 10, help = "how many hits to print, never above the cap of the layer")]
        limit: usize,
        #[arg(long, help = "print the wire json a face consumes instead of the text a person reads")]
        json: bool,
    },
    #[command(about = "answer the same wire json over loopback http, read only, for a face in a browser")]
    Serve {
        #[arg(long, default_value_t = serve::DEFAULT_PORT, help = "the loopback port to bind on both 127.0.0.1 and ::1")]
        port: u16,
    },
    #[command(about = "check this engine's own source: comments outside the header, and any call that turns a missing value into a default")]
    Selftest {
        #[arg(long, help = "the directory of rust files to scan; the crate's own src by default")]
        path: Option<String>,
    },
}

fn resolve_db(explicit: Option<&str>) -> PathBuf {
    match manifest::find_db(explicit) {
        Some(path) => path,
        None => {
            eprintln!(
                "UNTESTABLE: no DB directory was found. A DB is a directory holding {}; pass --db, set DB_DIR, or run inside one",
                manifest::DB_MANIFEST
            );
            exit(2);
        }
    }
}

fn load_or_refuse(db: &Path) -> gate::Corpus {
    match gate::load_corpus(db) {
        Ok(corpus) => corpus,
        Err(error) => {
            eprintln!("UNTESTABLE: {}", error);
            exit(2);
        }
    }
}

fn refuse_partial(corpus: &gate::Corpus) {
    if !corpus.band_failures.is_empty() {
        eprintln!(
            "UNTESTABLE: {} of {} band(s) named in bands.order do not carry a readable contract: {:?}",
            corpus.band_failures.len(),
            corpus.manifest.band_order.len(),
            corpus.band_failures
        );
        exit(2);
    }
    if !corpus.unreadable.is_empty() {
        eprintln!(
            "UNTESTABLE: {} declared document(s) could not be read, so an index built now would answer for a corpus nobody read: {:?}",
            corpus.unreadable.len(),
            corpus.unreadable
        );
        exit(2);
    }
    if !corpus.absent.is_empty() {
        eprintln!(
            "UNTESTABLE: {} declared path(s) are not on disk: {:?}",
            corpus.absent.len(),
            corpus.absent
        );
        exit(2);
    }
    if corpus.atoms.is_empty() {
        eprintln!(
            "UNTESTABLE: {} document(s) produced 0 atom(s); an empty scan is never reported as a pass",
            corpus.documents.len()
        );
        exit(2);
    }
}

fn compile_now(db: &Path, corpus: &gate::Corpus) -> store::IndexStats {
    refuse_partial(corpus);
    let journal = store::read_journal(db);
    if !journal.unparsable.is_empty() {
        eprintln!(
            "UNTESTABLE: {}; the journal is Source, so a line nobody can read is not a line that can be skipped",
            journal.denominator()
        );
        exit(2);
    }
    let digest = match store::source_digest(db, &corpus.manifest, &corpus.bands) {
        Ok(digest) => digest,
        Err(error) => {
            eprintln!("UNTESTABLE: the source could not be digested: {}", error);
            exit(2);
        }
    };
    let stats = match store::rebuild_index(
        db,
        &corpus.manifest,
        &corpus.bands,
        &corpus.documents,
        &corpus.atoms,
        &journal.records,
        &digest,
    ) {
        Ok(stats) => stats,
        Err(error) => {
            eprintln!("the index could not be built: {}", error);
            exit(2);
        }
    };
    let table = match store::open_index(db) {
        Ok(connection) => match store::table_digest(&connection) {
            Ok(table) => table,
            Err(error) => {
                eprintln!("the index was written but could not be digested: {}", error);
                exit(2);
            }
        },
        Err(error) => {
            eprintln!("the index was written but could not be reopened: {}", error);
            exit(2);
        }
    };
    let raw = match store::refresh_raw(db, &corpus.bands) {
        Ok(raw) => raw,
        Err(error) => {
            eprintln!("the raw tier could not be rebuilt: {}", error);
            exit(2);
        }
    };
    println!(
        "compiled {} band(s), {} document(s), {} atom(s), {} fts row(s), {} relation(s), {} journal row(s)",
        stats.bands, stats.documents, stats.atoms, stats.fts_rows, stats.relations, stats.journal_rows
    );
    println!(
        "raw tier: {} of {} document(s) copied under {} as {} byte addressed by their own digest",
        raw.written,
        raw.documents,
        store::raw_dir(db).display(),
        raw.bytes
    );
    println!("journal: {}", journal.denominator());
    println!("source_digest {}", digest);
    println!("raw_digest    {}", raw.digest);
    println!("table_digest  {}", table);
    println!(
        "the index is regenerable: delete {} and run db compile again; both digests are over the source and the tables, never over the clock",
        store::index_path(db).display()
    );
    stats
}

fn open_or_refuse(db: &Path) -> rusqlite::Connection {
    match store::open_index(db) {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("UNTESTABLE: {}", error);
            exit(2);
        }
    }
}

fn emit(outcome: route::Outcome, json: bool) -> ! {
    if json {
        println!("{}", route::wire(&outcome));
        exit(outcome.exit);
    }
    if outcome.exit == 0 {
        print!("{}", outcome.text);
    } else {
        eprint!("{}", outcome.text);
    }
    eprintln!("reason: {}", outcome.reason);
    exit(outcome.exit);
}

fn filters(
    band: Option<String>,
    layer: Option<String>,
    role: Option<String>,
    executor: Option<String>,
) -> route::Filters {
    route::Filters {
        band,
        layer,
        role,
        executor,
    }
}

fn argument_json(argument: &clap::Arg) -> serde_json::Value {
    let long = match argument.get_long() {
        Some(name) => serde_json::Value::String(format!("--{}", name)),
        None => serde_json::Value::Null,
    };
    let takes_value = !matches!(
        argument.get_action(),
        clap::ArgAction::SetTrue | clap::ArgAction::SetFalse | clap::ArgAction::Count
    );
    let value = match argument.get_value_names() {
        Some(names) if takes_value => serde_json::Value::String(
            names.iter().map(|name| format!("<{}>", name)).collect::<Vec<String>>().join(" "),
        ),
        _ => serde_json::Value::Null,
    };
    let help = match argument.get_help() {
        Some(text) => serde_json::Value::String(text.to_string()),
        None => serde_json::Value::Null,
    };
    serde_json::json!({
        "name": argument.get_id().as_str(),
        "positional": argument.get_long().is_none() && argument.get_short().is_none(),
        "long": long,
        "value": value,
        "required": argument.is_required_set(),
        "help": help,
    })
}

fn arguments_json(command: &clap::Command) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::new();
    for argument in command.get_arguments() {
        let id = argument.get_id().as_str();
        if id == "help" || id == "version" {
            continue;
        }
        out.push(argument_json(argument));
    }
    out
}

fn dump_commands() -> String {
    let root = Cli::command();
    let mut commands: Vec<serde_json::Value> = Vec::new();
    for sub in root.get_subcommands() {
        let about = match sub.get_about() {
            Some(text) => serde_json::Value::String(text.to_string()),
            None => serde_json::Value::Null,
        };
        commands.push(serde_json::json!({
            "name": sub.get_name(),
            "about": about,
            "arguments": arguments_json(sub),
        }));
    }
    let exits: Vec<serde_json::Value> = EXIT_CODES
        .iter()
        .map(|(code, meaning, when)| {
            serde_json::json!({ "code": code, "meaning": meaning, "when": when })
        })
        .collect();
    let document = serde_json::json!({
        "binary": root.get_name(),
        "about": match root.get_about() {
            Some(text) => serde_json::Value::String(text.to_string()),
            None => serde_json::Value::Null,
        },
        "global_arguments": arguments_json(&root),
        "commands": commands,
        "exit_codes": exits,
    });
    match serde_json::to_string_pretty(&document) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("the command tree could not be serialised: {}", error);
            exit(2);
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if cli.dump_commands {
        println!("{}", dump_commands());
        exit(0);
    }
    let command = match cli.command {
        Some(command) => command,
        None => {
            eprintln!("db needs a command, or --dump-commands. Run db --help for the list");
            exit(2);
        }
    };
    if let Command::Selftest { path } = &command {
        let dir = match path {
            Some(given) => PathBuf::from(given),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
        };
        let lines = gate::selftest_gates(&dir);
        print!("{}", gate::render(&lines));
        println!("scanned {}", dir.display());
        exit(gate::exit_for(&lines));
    }
    let db = resolve_db(cli.db.as_deref());

    match command {
        Command::Selftest { .. } => exit(2),
        Command::Compile => {
            let corpus = load_or_refuse(&db);
            compile_now(&db, &corpus);
            exit(0);
        }
        Command::Push => {
            let corpus = load_or_refuse(&db);
            refuse_partial(&corpus);
            let journal = store::read_journal(&db);
            if !journal.unparsable.is_empty() {
                eprintln!(
                    "UNTESTABLE: {}; nothing was appended, because appending onto lines nobody can read would extend a chain over a hole",
                    journal.denominator()
                );
                exit(2);
            }
            let stamp = store::now_stamp();
            let mut seq = journal.max_seq();
            let mut records: Vec<store::JournalRecord> = Vec::new();
            let mut unchanged = 0usize;
            let mut fresh = 0usize;
            let mut versioned = 0usize;
            let mut with_unknown = 0usize;
            for item in &corpus.atoms {
                let lineage = atom::lineage_of(item);
                let (version, supersedes) = match journal.newest_for_lineage(&lineage) {
                    Some(record) if record.atom_id == item.id => {
                        unchanged += 1;
                        continue;
                    }
                    Some(record) => {
                        versioned += 1;
                        (record.version + 1, vec![record.atom_id.clone()])
                    }
                    None => {
                        fresh += 1;
                        (1, Vec::new())
                    }
                };
                let verdict = if atom::undeclared_fields(item).is_empty() {
                    atom::VERDICT_PASS
                } else {
                    with_unknown += 1;
                    atom::VERDICT_UNKNOWN
                };
                seq += 1;
                records.push(store::JournalRecord {
                    seq,
                    ts: stamp.clone(),
                    atom_id: item.id.clone(),
                    lineage,
                    version,
                    prev_hash: String::new(),
                    gate_verdict: verdict.to_string(),
                    executor: item.executor.clone(),
                    supersedes,
                });
            }
            let appended = match store::append_journal(&db, &mut records) {
                Ok(count) => count,
                Err(error) => {
                    eprintln!("nothing was appended: {}", error);
                    exit(2);
                }
            };
            println!(
                "push over {} atom(s): {} new, {} a new version of an atom already admitted, {} unchanged and skipped",
                corpus.atoms.len(),
                fresh,
                versioned,
                unchanged
            );
            println!(
                "{} admission event(s) appended; {} of them carry the verdict {} because an attribute is undeclared, which is recorded and never folded into a failure",
                appended,
                with_unknown,
                atom::VERDICT_UNKNOWN
            );
            let corpus = load_or_refuse(&db);
            compile_now(&db, &corpus);
            exit(0);
        }
        Command::Gate => {
            let corpus = load_or_refuse(&db);
            let mut lines = gate::source_gates(&db, &corpus);
            lines.extend(gate::index_gates(&db, &corpus));
            match store::open_index(&db) {
                Ok(connection) => {
                    lines.extend(gate::query_gates(&connection, &corpus.manifest));
                    lines.push(gate::settings_gate(&db));
                    lines.extend(route::commute_gate(&connection));
                }
                Err(error) => {
                    lines.push(gate::GateLine::unknown(
                        "G-Q1",
                        gate::REASON_INDEX_ABSENT,
                        0,
                        0,
                        error.clone(),
                    ));
                    lines.push(gate::settings_gate(&db));
                    lines.push(gate::GateLine::unknown(
                        "G-Q3",
                        gate::REASON_INDEX_ABSENT,
                        0,
                        0,
                        error,
                    ));
                }
            }
            print!("{}", gate::render(&lines));
            let journal = store::read_journal(&db);
            println!("journal denominator: {}", journal.denominator());
            println!(
                "corpus denominator: {} band(s), {} document(s), {} atom(s)",
                corpus.bands.len(),
                corpus.documents.len(),
                corpus.atoms.len()
            );
            exit(gate::exit_for(&lines));
        }
        Command::Ls {
            band,
            layer,
            role,
            executor,
            lod,
            cursor,
            json,
        } => {
            let corpus = load_or_refuse(&db);
            let connection = open_or_refuse(&db);
            let outcome = route::ls(
                &connection,
                &corpus.manifest,
                &filters(band, layer, role, executor),
                lod,
                cursor.as_deref(),
            );
            emit(outcome, json);
        }
        Command::Show { address, lod, json } => {
            let connection = open_or_refuse(&db);
            emit(route::show(&connection, &address, lod), json);
        }
        Command::Find {
            needle,
            band,
            layer,
            limit,
            json,
        } => {
            let corpus = load_or_refuse(&db);
            let connection = open_or_refuse(&db);
            let outcome = route::find(
                &connection,
                &corpus.manifest,
                &needle,
                &filters(band, layer, None, None),
                limit,
            );
            emit(outcome, json);
        }
        Command::Serve { port } => {
            let corpus = load_or_refuse(&db);
            match store::open_index(&db) {
                Ok(_) => {}
                Err(error) => {
                    eprintln!("UNTESTABLE: {}", error);
                    exit(2);
                }
            }
            exit(serve::serve(&db, &corpus.manifest, port));
        }
    }
}
