#![forbid(unsafe_code)]
//! The `gx` binary. 44 §1's thirteen subcommands — **eight of them**.
//!
//! Hand 1 built the ground (the `.gx/` directory, the draft store, the id-resolution cache) and
//! implemented no verb. Hand 2 implemented 44 §1.2's read side (「`gx receipt show|verify` /
//! `gx log proof|consistency` / `gx key gen|list` / `gx replay` + offline verifier」). **Hand 3**
//! implements the pipeline's front half — 「`gx submit` / `gx plan` / `gx verify` / `gx commit`」 —
//! which is where the binary stops only reading, and where 44 §1.4's **2** becomes reachable for the
//! first time. `undo`, `cancel`, `escalation`, `policy` and `serve` are hands 4 onwards.
//!
//! # 🔴 規律52 — the exit code clap wanted is the one 44 gives to 「denied」
//!
//! req/38 §48 M6H1-1 採(a) made it a rule after hand 1 found it:
//!
//! > 全 subcommand は `try_parse()`+写像(usage error→exit **1**「入力不正」・`--help`/`--version`→0)。
//! > **44 §1.4 の exit 2 は状態機械の「拒否(denied)」専用であり、CLI 引数の usage error に clap 既定の
//! > 2 を流用しない**(E-M6-2)。clap 既定 `parse()` 禁。
//!
//! `Parser::parse()` appears nowhere below. [`gx_cli::exit::DENIED`] **is** returned now — by
//! `gx verify` on a `Verdict::Deny` and by `gx commit` on an un-admitted transformation, which are
//! the two things 44 §1.4 says the number means — and that is exactly why 規律52 exists: a binary
//! that also answered a mistyped flag with 2 would make the status useless to the script that reads
//! it. `crates/gx-cli/tests/exit_map.rs` measures the parser half and
//! `crates/gx-cli/tests/ac_054.rs` measures the state-machine half.
//!
//! # What goes where
//!
//! 44 §1.3 fixes the contract: a single newline-terminated JSON object on stdout, and refusals on
//! **stderr** as `{"type", "title", "gx_code", "detail"}` with stdout left empty. So every command
//! below returns an [`Outcome`] the library built, `main` prints it, and every `Err` becomes
//! `Error::problem()` on stderr. Notes an operator needs and a pipe does not — where a new key was
//! filed, for instance — also go to stderr, which is what keeps `gx key gen --json > key.pub.json`
//! a file containing exactly 44 §1.2's two fields.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use gx_cli::exit::{Outcome, ERROR, OK};
use gx_cli::keys::KeyStore;
use gx_cli::layout::Layout;
use gx_cli::receipt::{ReceiptStore, Source};
use gx_cli::session::Session;
use gx_cli::{clock, keys, ledger, receipt, replay, rng, Error, Result};
use gx_core::{Cid, EnforcementMode, TransformationId};

/// `gx` — the Glovrex command line.
#[derive(Debug, Parser)]
#[command(
    name = "gx",
    version,
    about = "Glovrex: transformations as first-class, verified objects.",
    long_about = "M6 hand 2 implements 44 §1.2's read side: receipt show/verify, log \
                  proof/consistency/checkpoint, key gen/list, replay. The write side (submit, plan, \
                  verify, commit, undo, cancel, escalation, policy, serve) is hands 3 onwards.",
    subcommand_required = true,
    arg_required_else_help = false
)]
struct Cli {
    /// The project whose `.gx/` directory to use. Defaults to the working directory.
    #[arg(long, global = true, value_name = "DIR")]
    project: Option<PathBuf>,

    /// 44 §1.3: 「CLIは`--pretty`で人間可読整形を追加提供」.
    #[arg(long, global = true)]
    pretty: bool,

    #[command(subcommand)]
    command: Command,
}

/// 44 §1.1's thirteen. Eight of the verbs are here.
#[derive(Debug, clap::Subcommand)]
enum Command {
    /// T-1: create a Draft from an intent (44 §1.2).
    Submit {
        /// Which substrate the change is against.
        #[arg(long, value_name = "fs|git|mcp")]
        substrate: String,
        /// The position inside it, in the adapter's own spelling (42 §3.3).
        #[arg(long, value_name = "STR")]
        locator: String,
        /// The intent body, or `-` for stdin. Carried opaquely — see [`intent_bytes`].
        #[arg(long, value_name = "FILE|-")]
        intent: String,
        /// 42 §3.2's `ChangeContext`.
        #[arg(
            long,
            value_name = "Time|Evidence|Policy|Model|Representation|Substrate|Custom:NAME"
        )]
        context: String,
        /// The key id of the actor asking (42 §3.2: the DSSE `keyid` namespace).
        #[arg(long, value_name = "KEY_ID")]
        actor_key: String,
        /// Which of 42 §3.2's three actors this is.
        #[arg(long, value_name = "human|agent|process", default_value = "human")]
        actor_kind: String,
        /// The model, for `--actor-kind agent`.
        #[arg(long, value_name = "STR")]
        actor_model: Option<String>,
        /// 44 §1.2's flag. v0.1 produces order 0 only; see `pipeline::submit`.
        #[arg(long, default_value_t = 0, value_name = "0|1|2")]
        order: u8,
        /// 44 §1.2's flag. v0.1 has one producer of parents and it is `undo`.
        #[arg(long, value_name = "TID")]
        parent: Vec<String>,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// T-2: snapshot and plan, fixing the `TransformationId` (44 §1.2).
    Plan {
        /// An `IntentId` or a `TransformationId` — 44 §0's id-resolution accepts both.
        id: String,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-3 → T-4: collect evidence, ask the gate (44 §1.2).
    Verify {
        /// The `gx1:` transformation id.
        transformation: String,
        /// Pre-collected `Evidence` (42 §3.7) as JSONL, added to what the gate computes.
        #[arg(long, value_name = "FILE")]
        evidence: Vec<PathBuf>,
        /// DR-2 record-only, **for this call** (M6-08 採(a)). Not a fail posture.
        #[arg(long)]
        record_only: bool,
        /// 🔴 Decide with this pack instead of the shipped one — not in 44 §1.2 (**E-M6-12**).
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-8 → T-11: canonicalize, re-check the precondition, apply, append, issue (44 §1.2).
    Commit {
        /// The `gx1:` transformation id.
        transformation: String,
        /// 44 §1.2: 「未指定時はCLIが`transformation_id`から決定的に導出」.
        #[arg(long, value_name = "STR")]
        idempotency_key: Option<String>,
        /// 🔴 Not in 44 §1.2's synopsis — see `run`'s note and M6H3-7.
        #[arg(long)]
        record_only: bool,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// Read and check receipts (44 §1.2).
    Receipt {
        #[command(subcommand)]
        cmd: ReceiptCmd,
    },
    /// Read the ledger (44 §1.2), and publish a signed head (M6-24 採(b)).
    Log {
        #[command(subcommand)]
        cmd: LogCmd,
    },
    /// Generate and list signing keys (44 §1.2, req/56 §3).
    Key {
        #[command(subcommand)]
        cmd: KeyCmd,
    },
    /// T-12: commit the escrowed inverse of a committed transformation (44 §1.2, P-5).
    Undo {
        /// The `gx1:` transformation id to take back.
        transformation: String,
        /// 44 §1.2's flag. v0.1 signs with the original actor's key — see `lifecycle::undo`.
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2: 「未指定時はCLIが`transformation_id`から決定的に導出」.
        #[arg(long, value_name = "STR")]
        idempotency_key: Option<String>,
        /// 🔴 Decide with this pack instead of the shipped one — not in 44 §1.2 (**E-M6-12**).
        ///
        /// An undo is verified like anything else (43 §5-2), so it has a verdict, and the verdict
        /// can be `Deny` — 44 §1.4's 2. Reaching that from a `gx` invocation needs a pack that
        /// refuses a **writable** path, for the same reason `gx verify --policy` needs one.
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// T-7: the owner stops a transformation before the critical section (44 §1.2, DR-11).
    Cancel {
        /// The `gx1:` transformation id.
        transformation: String,
        /// 44 §1.2's flag. v0.1 has no authorization layer — see `lifecycle::cancel`.
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-5 / T-5b: a person rules on an escalated transformation (44 §1.2, DR-11).
    Escalation {
        #[command(subcommand)]
        cmd: EscalationCmd,
    },
    /// Check a Cedar policy pack, and run scenarios against it (44 §1.2).
    Policy {
        #[command(subcommand)]
        cmd: PolicyCmd,
    },
    /// Deterministic replay of the engine journal (44 §1.2, E-M5-2).
    Replay {
        /// A `gx1:` transformation id. Omit it and use `--from`/`--to`, or neither for everything.
        transformation: Option<String>,
        /// First journal record index, inclusive (M6H2-8: of the **journal**).
        #[arg(long, requires = "to")]
        from: Option<usize>,
        /// Last journal record index, exclusive.
        #[arg(long, requires = "from")]
        to: Option<usize>,
        /// Accepted for 44 §1.2's synopsis. Replay writes nothing (E-M5-2); see M6H2-9.
        #[arg(long)]
        dry_run: bool,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Put down an intent that was never planned (**E-M6-14**; not in 44 §1.1's thirteen).
    ///
    /// The verb exists because **E-M6-1** took `Draft` out of `gx cancel`'s from-set: a draft has no
    /// `TransformationId`, no row in the state table (M5-17 採(b)) and no journal record that could
    /// carry an `Aborted`. 「draft 破棄は台帳に載らない操作」 — 43 T-7 is a transition and this is not.
    Draft {
        #[command(subcommand)]
        cmd: DraftCmd,
    },
    /// Serve 44 §2's HTTP surface until a signal (44 §1.1: 「`gx serve` | gx-api起動」).
    ///
    /// 🔴 **Authorization** — the only check is a single static Bearer token (44 §2.5, v0.1). It
    /// answers whether the caller holds this server's token and nothing about who they are: there is
    /// no authorization layer in v0.1 (M5H6-4), `cancel` and `escalation` accept the actor the
    /// request declares, and 43 T-7's owner guard has no enforcement point. The default bind is
    /// therefore loopback (127.0.0.1:8787); binding anywhere else exposes an unauthorized surface and
    /// is refused without an explicit flag.
    Serve {
        /// 44 §1.2's flag. Loopback only in v0.1 — see the note above (M6-10 採(b)).
        #[arg(long, value_name = "ADDR:PORT")]
        bind: Option<String>,
        /// DR-2's `EnforcementMode` axis for this process (43 §4). **Not** a fail posture.
        #[arg(long)]
        record_only: bool,
        /// DR-2's other axis (ASM-13): what happens when the verifier cannot be reached.
        #[arg(long, value_name = "closed|open")]
        fail_posture: Option<String>,
        /// 44 §1.2's synopsis. Refused in v0.1 — 44 §2.5 puts mTLS in 「v0.2（予告）」 (N-09).
        #[arg(long, value_name = "PATH")]
        tls_cert: Option<PathBuf>,
        /// As `--tls-cert`.
        #[arg(long, value_name = "PATH")]
        tls_key: Option<PathBuf>,
        /// 🔴 The file holding 44 §2.5's bearer token. Required; the **path** is the argument so
        /// that the secret is not in `ps` (not in 44 §1.2's synopsis — M6H6-8).
        #[arg(long, value_name = "PATH")]
        token_file: Option<PathBuf>,
        /// The key this server signs receipts with (45 §1). Defaults to `.gx/config.toml`'s
        /// `engine_signing_keyid` (**E-M6-7**).
        #[arg(long, value_name = "KEY_ID")]
        signing_key: Option<String>,
    },
}

#[derive(Debug, clap::Subcommand)]
enum DraftCmd {
    /// 🔴 **E-M6-14** — discard a draft body. Writes nothing to the ledger.
    Discard {
        /// The `gx1:` **intent** id `gx submit` printed.
        intent: String,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum EscalationCmd {
    /// T-5 — 「人間裁定 = Admit」, to `Admitted` (AC-071).
    Approve {
        /// A `TicketId`, or the `TransformationId` 44 §2.2 uses for the same operation (M6-04 採(c)).
        id: String,
        /// 44 §1.2: 「`--reason`: 裁定理由（必須）」.
        #[arg(long, value_name = "TEXT")]
        reason: String,
        /// The ruler's key id. 43 T-5: 「裁定者が有効な署名鍵を保持」.
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-5b — 「人間裁定 = Deny」, to `Denied` (AC-072).
    Reject {
        /// A `TicketId`, or the `TransformationId` 44 §2.2 uses (M6-04 採(c)).
        id: String,
        /// 44 §1.2: 「`--reason`: 裁定理由（必須）」.
        #[arg(long, value_name = "TEXT")]
        reason: String,
        /// The ruler's key id.
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum PolicyCmd {
    /// 44 §1.2: 「Cedar policy構文・スキーマ検証」 — and the invariant half (FR-027, M6-21).
    Lint {
        /// The pack to read.
        path: PathBuf,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 44 §1.2: 「指定シナリオ…に対しGate評価を実行し期待値と照合」.
    Test {
        /// The pack to decide with.
        path: PathBuf,
        /// The scenarios: `Intent`/`Evidence`/expected `Verdict`, as JSON.
        #[arg(long, value_name = "FILE")]
        scenario: PathBuf,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum ReceiptCmd {
    /// Show a stored receipt, at one of 48 §3.1's four disclosure levels (M6-16 採(a)).
    Show {
        /// The `gx1:` transformation id.
        transformation: String,
        /// 1=verdict badge, 2=summary, 3=full expansion, 4=raw signatures. Default 1.
        #[arg(long, default_value_t = 1, value_name = "1..4")]
        level: u8,
        /// 「`--json` は常に全量」 (M6-16 採(a)): equivalent to `--level 4`.
        #[arg(long)]
        json: bool,
    },
    /// Verify a receipt: DSSE signature, canonical CID, and inclusion against a known checkpoint.
    Verify {
        /// The receipt file, or `-` for stdin (44 §1.2).
        file: String,
        /// Do not consult any ledger. AC-057's mode.
        #[arg(long)]
        offline: bool,
        /// The known `Checkpoint` to check the inclusion proof against.
        #[arg(long, value_name = "FILE")]
        checkpoint: Option<PathBuf>,
        /// 🔴 The key the `--checkpoint` was signed with (**M6H8-11 採(b)**, req/38 §55). Without it
        /// the anchor is taken on trust and the answer says `anchor_authenticated: false`.
        #[arg(long, value_name = "FILE")]
        checkpoint_key: Option<PathBuf>,
        /// The public key. `gx key gen`'s output, or a gx key file. See M6H2-6.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// 🔴 **FR-M7-3**: the revocation list to consult (`gx key revoke`'s output). Without it the
        /// answer says `revocation: not_consulted`, which ASM-45-2 permits — 「revocation list参照は
        /// verifier側任意」 — and which is a different word from `not_revoked`.
        #[arg(long, value_name = "FILE")]
        revocations: Option<PathBuf>,
        /// 🔴 How far back a revocation reaches. ASM-45-2's DEFAULT is `from-revocation`
        /// (「失効前に発行済みのreceiptは遡及無効化しない」); `all` refuses every receipt the key ever
        /// signed and is the setting a compromise is answered with, because it reads no clock.
        #[arg(
            long,
            value_name = "from-revocation|all",
            default_value = "from-revocation"
        )]
        retroaction: String,
    },
}

#[derive(Debug, clap::Subcommand)]
enum LogCmd {
    /// An inclusion proof for one leaf (44 §1.2).
    Proof {
        /// A leaf index or a `gx1:` transformation id.
        #[arg(long, value_name = "INDEX|TID")]
        leaf: String,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// A consistency proof between two tree sizes (44 §1.2).
    Consistency {
        #[arg(long, value_name = "SIZE")]
        from: u64,
        #[arg(long, value_name = "SIZE")]
        to: u64,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Publish a signed checkpoint of the current tree (**M6-24 採(b)**; not in 44 §1.1, M6H2-7).
    Checkpoint {
        /// The ledger signing key. Required: only the log's owner can publish its head.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// The log's namespace (42 §3.11).
        #[arg(long, default_value = ledger::DEFAULT_ORIGIN)]
        origin: String,
        /// Also write the checkpoint here, byte for byte as stdout carries it.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, clap::Subcommand)]
enum KeyCmd {
    /// Generate an Ed25519 signing key (44 §1.2, FR-020).
    Gen {
        #[arg(long, default_value = keys::ALGORITHM)]
        alg: String,
        /// Where to write the secret. Defaults to req/56 §3's `~/.gx/keys/<key_id>.key`.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// 🔴 **FR-M7-4**: record the generated id in the project's `.gx/config.toml` as
        /// `engine_signing_keyid`, which is where `gx serve` reads it (E-M6-7). Without this flag
        /// the slot has a reader and no writer (M6H7-8), and a fresh volume has no way to fill it.
        #[arg(long)]
        record: bool,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// List locally known key ids (44 §1.2).
    List {
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Revoke a key (**FR-M7-3**, 裁定 #6; not in 44 §1.2 — see `gx_cli::keys::revoke`).
    Revoke {
        /// The key to revoke. Its secret must still be in the store: a revocation is signed by the
        /// key it revokes.
        #[arg(long, value_name = "KEY_ID")]
        key_id: String,
        /// Why, in the operator's words. It travels inside the signed entry.
        #[arg(long, default_value = "revoked by the operator")]
        reason: String,
        /// Write the list here instead of `~/.gx/keys/revocations.json`.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Generate a successor and revoke the predecessor, in one command (**FR-M7-3**).
    Rotate {
        /// The key being rotated out.
        #[arg(long, value_name = "KEY_ID")]
        key_id: String,
        #[arg(long, default_value = keys::ALGORITHM)]
        alg: String,
        #[arg(long, default_value = "rotated by the operator")]
        reason: String,
        /// Record the **successor**'s id in `.gx/config.toml` (FR-M7-4). A rotation whose server
        /// keeps signing with the revoked key is a rotation that did nothing.
        #[arg(long)]
        record: bool,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // `--help` and `--version` arrive here as errors and are not errors: clap models them as
            // a parse that terminated early, and 44 §1.4's 0 is 「正常終了」.
            //
            // 🔴 `DisplayHelpOnMissingArgumentOrSubcommand` is **not** in the set, and hand 1's
            // mapping had it there. The two are different events: 「the operator asked for help」 and
            // 「the operator named a verb that needs a sub-verb and gave none」. With one level of
            // subcommands the distinction never fired; with two, `gx receipt` took it — a command
            // that did nothing and exited 0. 44 §1.4's 0 is 「目的の状態に到達」 and no state was
            // reached, so it is 1. Raised as **M6H2-11**.
            let asked_for_output = matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            let _ = e.print();
            // 🔴 Not `e.exit()`, which would use clap's 2. 規律52.
            return ExitCode::from(if asked_for_output { OK } else { ERROR });
        }
    };

    let pretty = cli.pretty;
    match run(&cli) {
        Ok(outcome) => {
            print_json(&outcome.json, pretty);
            ExitCode::from(outcome.code)
        }
        Err(e) => {
            // 44 §1.3: 「エラーはstderrに…を出力し、stdoutは何も出さない（または部分結果を出さない）」.
            eprintln!(
                "{}",
                serde_json::to_string(&e.problem()).unwrap_or_default()
            );
            ExitCode::from(e.exit_code())
        }
    }
}

/// 44 §1.3: 「単一オブジェクトを返すコマンドはstdoutに**改行終端の単一JSON**を出力する」.
fn print_json(value: &serde_json::Value, pretty: bool) {
    let text = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    };
    println!("{}", text.unwrap_or_default());
}

/// The project root every `.gx/`-bound command resolves against.
fn project(cli: &Cli) -> Result<PathBuf> {
    match &cli.project {
        Some(p) => Ok(p.clone()),
        None => std::env::current_dir().map_err(|e| Error::Io {
            action: "read",
            path: ".".to_string(),
            source: e,
        }),
    }
}

/// Parse 42 §1.2's `gx1:<base32>` into a transformation id.
///
/// [`Cid::from_text`] and never a mint — 則 1 (i). The parser is in gx-core exactly so that this
/// line does not have to reach gx-canon (see the crate root).
fn transformation_id(text: &str) -> Result<TransformationId> {
    Cid::from_text(text)
        .map(TransformationId)
        .map_err(|e| Error::Usage {
            detail: format!("`{text}` is not a `gx1:` id: {e}"),
        })
}

/// 42 §3.1's four substrates, from `--substrate`. The spelling is [`gx_cli::substrate_kind`]'s:
/// `gx policy test`'s scenario file reads the same names and 44 §1.2 gives them one vocabulary.
fn substrate(text: &str) -> Result<gx_core::SubstrateKind> {
    gx_cli::substrate_kind(text)
}

/// 42 §3.2's `ChangeContext`, from `--context`. Shared for [`substrate`]'s reason.
fn context(text: &str) -> Result<gx_core::ChangeContext> {
    gx_cli::change_context(text)
}

/// 42 §3.2's `Actor`, from `--actor-kind` / `--actor-key` / `--actor-model`.
///
/// 🔴 `--actor-model` is **required** for an agent and refused for the other two. 42 §3.2: 「`Agent`
/// adds `model` because that is the one fact about an agent a human reviewer needs and cannot
/// recover from the key」 — a default would put a made-up model in a signed provenance record, which
/// is the shape M5H4-4 refused for `adapter_version`.
fn actor(kind: &str, key: &str, model: Option<&str>) -> Result<gx_core::Actor> {
    use gx_core::Actor as A;
    let key = key.to_string();
    match (kind, model) {
        ("human", None) => Ok(A::Human { key }),
        ("process", None) => Ok(A::Process { key }),
        ("agent", Some(model)) => Ok(A::Agent {
            key,
            model: model.to_string(),
        }),
        ("agent", None) => Err(Error::Usage {
            detail:
                "--actor-kind agent needs --actor-model (42 §3.2: the one fact about an agent a \
                     reviewer cannot recover from the key)"
                    .to_string(),
        }),
        ("human" | "process", Some(_)) => Err(Error::Usage {
            detail: "--actor-model belongs to `--actor-kind agent`; 42 §3.2 gives `Human` and \
                     `Process` a key and nothing else"
                .to_string(),
        }),
        (other, _) => Err(Error::Usage {
            detail: format!("--actor-kind takes human|agent|process (44 §1.2); got {other:?}"),
        }),
    }
}

/// 🔴 The bytes `--intent <FILE|->` carried, **unparsed**.
///
/// 44 §1.2 calls it 「Intent JSON（42 §3.3の`goal`フィールド相当のJSON body）」 and this reads it as
/// bytes. The reason is **E-M4-2**: `GoalBytes` is opaque, and 「what these bytes mean」 is the one
/// interpretation P-6 reserves to an adapter — the fs adapter reads them as a file's new content and
/// says so in its own module. A CLI that parsed the body would be the second place in the system
/// that has an opinion about a goal, and the first place a JSON re-serialisation could change the
/// `IntentId` (42 §3.3 puts the goal in the identity). Raised as **M6H3-8**: 44 says JSON and this
/// stores whatever it was given.
fn intent_bytes(path: &str) -> Result<Vec<u8>> {
    if path == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| Error::Io {
                action: "read",
                path: "<stdin>".to_string(),
                source: e,
            })?;
        return Ok(buf);
    }
    std::fs::read(path).map_err(|e| Error::Io {
        action: "read",
        path: path.to_string(),
        source: e,
    })
}

fn run(cli: &Cli) -> Result<Outcome> {
    match &cli.command {
        Command::Submit {
            substrate: kind,
            locator,
            intent,
            context: ctx,
            actor_key,
            actor_kind,
            actor_model,
            order,
            parent,
            json: _,
        } => {
            // Arguments first, project second — hand 2's order, for hand 2's reason: a locator that
            // is not a locator is 「入力不正」 wherever it was typed.
            let spec = gx_cli::pipeline::SubmitSpec {
                substrate: substrate(kind)?,
                locator: locator.clone(),
                goal: intent_bytes(intent)?,
                context: context(ctx)?,
                actor: actor(actor_kind, actor_key, actor_model.as_deref())?,
                order: *order,
                parents: parent
                    .iter()
                    .map(|p| transformation_id(p))
                    .collect::<Result<Vec<_>>>()?,
            };
            // 🔴 `gx submit` is the one verb that **creates** `.gx/`. 44 has no `gx init`, and the
            // alternative — every verb creating — would answer a mistyped directory by starting a
            // second, empty ledger beside the operator's real one.
            let mut session = Session::open(&project(cli)?, true, Vec::new(), None)?;
            // 則 2: the clock and the entropy source, each read in one place, at the outside edge.
            gx_cli::pipeline::submit(&mut session, &spec, rng::seed(), clock::now())
        }
        Command::Plan { id, json: _ } => {
            let mut session = Session::open(&project(cli)?, false, Vec::new(), None)?;
            gx_cli::pipeline::plan(&mut session, id, clock::now())
        }
        Command::Verify {
            transformation,
            evidence,
            record_only,
            policy,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            let evidence = gx_cli::pipeline::read_evidence(evidence)?;
            // 🔴 **E-M6-12** — `--policy` is not in 44 §1.2's synopsis. req/38 §50 M6H3-9 採(a)
            // adds it, and `Session::open_with_policy` carries the reason: without a pack that
            // denies a **writable** path, DR-2's record-only commit could only be exercised
            // against `/etc`, which is a write to `/etc`.
            let mut session = Session::open_with_policy(
                &project(cli)?,
                false,
                evidence,
                None,
                policy.as_deref(),
            )?;
            // 44 §1.2: 「本コマンド単位で強制（グローバル設定の上書き）」 — the per-call argument of
            // M6-08 採(a), not `with_mode`.
            let mode = record_only.then_some(EnforcementMode::RecordOnly);
            gx_cli::pipeline::verify(&mut session, &id, mode, clock::now())
        }
        Command::Commit {
            transformation,
            idempotency_key,
            record_only,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            // 🔴 **M6H3-7**: `--record-only` is not in 44 §1.2's `gx commit` synopsis, and 44 §1.2's
            // [DR-2感度] paragraph for this command is normative — 「record-onlyモード下でVerdict=Deny
            // の対象を`commit`した場合、適用は通すが`Receipt.enforced=false`が刻まれ、exit codeは0」.
            // The mode that paragraph is about reaches T-8r through `canonicalize`, which `gx commit`
            // drives, and the only inputs 44 gives a single-shot CLI are per-command flags; without
            // one the paragraph names an outcome no `gx` invocation can produce. This is the shape
            // **E-M6-4** already ruled once (M6H2-6's `--key`, without which FR-052 was
            // unconstructible), and it is raised rather than assumed.
            //
            // 🔴 Hand 3 wrote this as `with_mode` and said why: 「`canonicalize` reads the engine's
            // setting, and a second per-call parameter is a ruling this hand does not have」. **This
            // hand has it** — E-M6-20 (req/38 §52) put `record_only` in 44 §2.2's commit body, and a
            // long-lived server has no other road to T-8r (M6-08 ruled the mode-swap form 「採っては
            // ならない」). So `canonicalize` now takes the argument `verify` already took, and both
            // surfaces drive one spelling: a per-call override, never a setting on shared state.
            let mode = record_only.then_some(EnforcementMode::RecordOnly);
            let mut session = Session::open(&project(cli)?, false, Vec::new(), None)?;
            gx_cli::pipeline::commit(
                &mut session,
                &id,
                idempotency_key.as_deref(),
                mode,
                clock::now(),
            )
        }
        Command::Undo {
            transformation,
            actor_key,
            idempotency_key,
            policy,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            // 🔴 M6H3-5's rule one verb along: a flag with nowhere to go is refused, not dropped.
            // `Engine::undo` mints T_u's `Intent` with **T_o's** context and actor (P-5), so the
            // key that signs is the original actor's and `--actor-key` selects nothing (M6H4-3).
            if let Some(key) = actor_key {
                return Err(Error::Usage {
                    detail: format!(
                        "--actor-key {key:?} has nowhere to go: 43 §5-1 makes T_u's intent carry \
                         T_o's own context and actor, so the receipt is signed with the original \
                         actor's key and this flag would select nothing (M6H4-3)"
                    ),
                });
            }
            let mut session = Session::open_with_policy(
                &project(cli)?,
                false,
                Vec::new(),
                None,
                policy.as_deref(),
            )?;
            gx_cli::lifecycle::undo(
                &mut session,
                &id,
                idempotency_key.as_deref(),
                rng::seed(),
                clock::now(),
            )
        }
        Command::Cancel {
            transformation,
            actor_key,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            // 43 T-7's owner guard has no enforcement point in v0.1 (M5H6-4 採(a)): `Engine::cancel`
            // takes no actor and the `Aborted` record has no field for one. Accepting the flag and
            // dropping it would tell an operator a permission was checked.
            if let Some(key) = actor_key {
                return Err(Error::Usage {
                    detail: format!(
                        "--actor-key {key:?} has nowhere to go: v0.1 has no authorization layer \
                         (M5H6-4 採(a)), `Engine::cancel` takes no actor and 43 T-7's `Aborted` \
                         record has no field for one, so nothing would check the permission the \
                         flag names (M6H4-3)"
                    ),
                });
            }
            let mut session = Session::open(&project(cli)?, false, Vec::new(), None)?;
            gx_cli::lifecycle::cancel(&mut session, &id, clock::now())
        }
        Command::Escalation { cmd } => escalation_cmd(cli, cmd),
        Command::Policy { cmd } => match cmd {
            PolicyCmd::Lint { path, json: _ } => gx_cli::policy::lint(path),
            PolicyCmd::Test {
                path,
                scenario,
                json: _,
            } => gx_cli::policy::test(path, scenario),
        },
        Command::Draft { cmd } => draft_cmd(cli, cmd),
        Command::Serve {
            bind,
            record_only,
            fail_posture,
            tls_cert,
            tls_key,
            token_file,
            signing_key,
        } => serve_cmd(
            cli,
            &gx_cli::serve::ServeSpec {
                bind: bind.clone(),
                record_only: *record_only,
                fail_posture: fail_posture.clone(),
                tls: (tls_cert.clone(), tls_key.clone()),
                token_file: token_file.clone(),
                signing_key: signing_key.clone(),
            },
        ),
        Command::Receipt { cmd } => receipt_cmd(cli, cmd),
        Command::Log { cmd } => log_cmd(cli, cmd),
        Command::Key { cmd } => key_cmd(cli, cmd),
        Command::Replay {
            transformation,
            from,
            to,
            dry_run,
            json: _,
        } => {
            let layout = Layout::open(&project(cli)?)?;
            let journal = replay::open(&layout)?;
            // The ledger is the independent witness `matches` is about (see the module header). Its
            // absence is reported rather than treated as agreement.
            let store = ledger::open(&layout).ok();
            let range = match (transformation, from, to) {
                (Some(tid), _, _) => replay::Range::Transformation(transformation_id(tid)?),
                (None, Some(from), Some(to)) => replay::Range::Records {
                    from: *from,
                    to: *to,
                },
                (None, None, None) => replay::Range::All,
                (None, _, _) => {
                    return Err(Error::Usage {
                        detail: "--from and --to come as a pair (44 §1.2)".to_string(),
                    })
                }
            };
            replay::replay(&journal, store.as_ref(), &range, *dry_run)
        }
    }
}

/// 🔴 **E-M6-14** — `gx draft discard <IntentId>`, the verb E-M6-1 made necessary.
///
/// 44 L101's `gx cancel` from-set once contained `Draft`; §47 M6-03 採(c) took it out, because a
/// draft has no `TransformationId` to cancel (req/88 Λ3: 「id-resolution は『指す』問題を解くが『席が
/// 無い』問題を解かない」). What was left was an intent an operator had submitted and could not put
/// down.
///
/// 🔴 **The ledger does not learn about it, and that is the ruling rather than an omission.**
/// M5H6-1 refused a fourteenth journal record (`Aborted{intent_id, OwnerCancelled}`) on the grounds
/// that the vocabulary would grow with nothing to protect, and §51 M6H4-2 採(a) fixed the verb with
/// 「『draft 破棄は台帳に載らない操作』を doc に 1 行」. Here is that line, and
/// `crates/gx-cli/tests/m6h6_cli.rs` is the count that keeps it true: a discard removes a file and
/// appends no record. What is discarded was never a transformation — 42 §1.3-3's state table starts
/// at `Candidate` — so there is nothing about it for a台帳 to be missing.
///
/// A draft that is not there is 44 §1.4's **6**, never 0: answering 「done」 for a name the project
/// never held is 「skip と pass を同じ顔にするな」 (req/29 §4) at the verb level.
fn draft_cmd(cli: &Cli, cmd: &DraftCmd) -> Result<Outcome> {
    let DraftCmd::Discard { intent, .. } = cmd;
    let intent_id = gx_core::IntentId(Cid::from_text(intent).map_err(|e| Error::Usage {
        detail: format!("`{intent}` is not a `gx1:` intent id: {e}"),
    })?);
    let layout = Layout::open(&project(cli)?)?;
    let drafts = gx_cli::draft::DraftStore::in_layout(&layout);
    let path = drafts.path_of(&intent_id);
    if !drafts.remove(&intent_id)? {
        return Err(Error::NotFound {
            what: "draft",
            id: intent.clone(),
        });
    }
    // 🔴 The index entry goes too. `.gx/index/` is req/56 §2's 「derived・消して良い」 cache of
    // `IntentId → TransformationId`, and a resolution pointing at a body that is gone would make
    // `gx plan` answer 「the draft is missing」 for an intent the operator deliberately discarded —
    // a 6 that reads like corruption. Best effort, because a cache that will not be written is not
    // a reason to fail an operation that succeeded.
    let (mut index, _) = gx_cli::index::ResolutionIndex::load(&layout);
    index.forget(&intent_id);
    let _ = index.store(&layout);
    Ok(Outcome::ok(serde_json::json!({
        "intent_id": intent_id.0.to_text(),
        "discarded": true,
        "path": path.display().to_string(),
        // The sentence the ruling asks for, on the wire as well as in the source: an operator
        // reading stdout is told that nothing was recorded, rather than having to infer it.
        "ledger": "unchanged: discarding a draft is not a transition (E-M6-14; 43 T-7 acts on a                    transformation and a draft is not one yet)",
    })))
}

/// 🔴 `gx serve` (44 §1.1) — build the four things a server needs, then run until a signal.
///
/// The judgement about which crate holds what is in `gx_cli::serve`'s module header, and the runtime
/// is `gx_api::serve`: this function is the part 44 §1.3 makes gx-cli's, which is what reaches
/// **stdout**. Two structured lines, one at each end (「stdout: 起動ログ（構造化JSON行）」), and the
/// second one carries the exit — a shutdown that abandoned work says so in the log **and** in the
/// status, because 44's exit 0 is 「正常終了」 and M4H4-2 forbids spelling a crash path as one.
fn serve_cmd(cli: &Cli, spec: &gx_cli::serve::ServeSpec) -> Result<Outcome> {
    let store = KeyStore::user_default()?;
    let (state, bind) = gx_cli::serve::build(&project(cli)?, &store, spec)?;
    let signing = state.keys().signing().key_id().clone();
    let outcome = gx_api::serve(state, &gx_cli::serve::config(bind), |addr| {
        // 44 §1.2's start-up log, printed **after** the listener exists: a line that said 「serving
        // on」 before the bind succeeded would be a line an operator's script could act on before
        // the socket was there.
        println!("{}", gx_cli::serve::start_line(addr, spec, &signing));
    })
    .map_err(|e| Error::Usage {
        detail: e.to_string(),
    })?;
    let code = outcome.exit_code();
    let json = outcome.to_json();
    Ok(if code == OK {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, code)
    })
}

/// 43 T-5 / T-5b, from the command line.
///
/// The ruler is 42 §3.2's `Actor` and is **not** the transformation's own: 42 §3.13's `HumanDecision`
/// carries 「the ruler, which is not `Transformation.actor` (the submitter)」, because a record naming
/// only the submitter would say who asked and never who allowed. `--actor-key` therefore *does* have
/// somewhere to go here, unlike on `undo` and `cancel`, and when it is absent the ruling is filed
/// under the process's own key store default rather than being invented.
fn escalation_cmd(cli: &Cli, cmd: &EscalationCmd) -> Result<Outcome> {
    let (id, reason, actor_key, decision) = match cmd {
        EscalationCmd::Approve {
            id,
            reason,
            actor_key,
            ..
        } => (id, reason, actor_key, gx_cli::lifecycle::Decision::Approve),
        EscalationCmd::Reject {
            id,
            reason,
            actor_key,
            ..
        } => (id, reason, actor_key, gx_cli::lifecycle::Decision::Reject),
    };
    // 44 §1.2 makes `--reason` mandatory and `Engine::escalation` refuses an empty one; the refusal
    // is repeated here so that it arrives as 「入力不正」 before a project is opened rather than as an
    // engine error after one (the ordering hand 2 fixed for every other verb).
    if reason.trim().is_empty() {
        return Err(Error::Usage {
            detail: "--reason is required and cannot be blank (44 §1.2: 「裁定理由（必須）」); \
                     AC-071/072 both ask the reason to reach the trail, and a ruling that says \
                     nothing is a ruling nobody can audit"
                .to_string(),
        });
    }
    // 🔴 **M6H4-6** — 44 §1.2 writes `[--actor-key <KEY_ID>]` as optional and this refuses without
    // it. 42 §3.13's `HumanDecision.actor` is 「the ruler, which is **not** `Transformation.actor`
    // (the submitter)」, and INV-S6 exists so that an escalation records **who allowed it**. There is
    // no honest default for who a person is: falling back to the submitter's key would file a
    // ruling under the name of the party the ruling is about, which is the one thing T-5 is for.
    let Some(key_id) = actor_key.clone() else {
        return Err(Error::Usage {
            detail: "--actor-key names the ruler and is required here, although 44 §1.2 writes it \
                     optional (M6H4-6). 42 §3.13's `HumanDecision` carries the **ruler** and not \
                     the submitter, and INV-S6 is why: an escalation records who allowed a change, \
                     so defaulting to the submitter's key would file the ruling under the name of \
                     the party it is about. `gx key list` shows the ids this store holds"
                .to_string(),
        });
    };
    let ruler = gx_core::Actor::Human { key: key_id };
    let mut session = Session::open(&project(cli)?, false, Vec::new(), None)?;
    gx_cli::lifecycle::escalation(&mut session, id, decision, reason, &ruler, clock::now())
}

fn receipt_cmd(cli: &Cli, cmd: &ReceiptCmd) -> Result<Outcome> {
    match cmd {
        ReceiptCmd::Show {
            transformation,
            level,
            json,
        } => {
            // 🔴 The arguments are parsed **before** the project is opened. An id that is not an id
            // is 「入力不正」 wherever it was typed, and a version that answered 「no `.gx/` here」
            // first would report the wrong one of two independent faults — and would make the
            // refusal depend on the working directory, which is how 「it works on my machine」 gets
            // into an error message.
            let id = transformation_id(transformation)?;
            let level = if *json { receipt::MAX_LEVEL } else { *level };
            let layout = Layout::open(&project(cli)?)?;
            let store = ReceiptStore::in_layout(&layout);
            receipt::show(&store, &id, level)
        }
        ReceiptCmd::Verify {
            file,
            offline,
            checkpoint,
            checkpoint_key,
            key,
            revocations,
            retroaction,
        } => {
            // 🔴 Read the receipt **before** anything is resolved from it. AC-057's environment has
            // no project directory at all, and a verifier that opened `.gx/` on the way in would
            // fail in the one place it has to work.
            let mut stdin_bytes = Vec::new();
            let source = if file == "-" {
                std::io::stdin()
                    .read_to_end(&mut stdin_bytes)
                    .map_err(|e| Error::Io {
                        action: "read",
                        path: "<stdin>".to_string(),
                        source: e,
                    })?;
                Source::Stdin(&stdin_bytes)
            } else {
                Source::File(std::path::Path::new(file))
            };

            let public = match key {
                Some(path) => keys::read_public(path)?,
                None => {
                    // The owner's convenience path: the key id the receipt declares, in the local
                    // store. Never available to the third party AC-057 is about, which is why the
                    // flag exists at all.
                    let raw = match &source {
                        Source::File(p) => std::fs::read(p).map_err(|e| Error::Io {
                            action: "read",
                            path: p.display().to_string(),
                            source: e,
                        })?,
                        Source::Stdin(b) => (*b).to_vec(),
                    };
                    let receipt: gx_witness::Receipt =
                        serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
                            what: "receipt",
                            path: file.clone(),
                            detail: detail.to_string(),
                        })?;
                    let key_id = receipt.payload()?.key_id;
                    KeyStore::user_default()?.load(&key_id)?.public()
                }
            };

            let (anchor, anchor_source) = if *offline {
                match checkpoint {
                    Some(path) => (Some(receipt::read_checkpoint(path)?), "checkpoint-file"),
                    // 44 §1.2 permits `--offline` alone. With no anchor a `CommitReceipt` reports
                    // `inclusion: unanchored`, which `Checks::verified` refuses to call a pass
                    // (H5-9) — so this is not a quiet downgrade, it is a visible one.
                    None => (None, "none"),
                }
            } else {
                match checkpoint {
                    Some(path) => (Some(receipt::read_checkpoint(path)?), "checkpoint-file"),
                    None => {
                        let layout = Layout::open(&project(cli)?)?;
                        let store = ledger::open(&layout)?;
                        (
                            Some(ledger::local_head(&store, clock::now())?),
                            "local-ledger",
                        )
                    }
                }
            };
            // 🔴 **M6H8-11 採(b)** (req/38 §55): the anchor's own signature, checked only when a key
            // for it is offered — and reported either way by `anchor_authenticated`. 45 ASM-45-1
            // allows the log's key to differ from the receipt's, so this is a second flag rather
            // than a reuse of `--key`.
            let anchor_authenticated = match (checkpoint_key, anchor.as_ref()) {
                (Some(path), Some(head)) => {
                    let anchor_key = keys::read_public(path)?;
                    match receipt::authenticate_anchor(head, &anchor_key) {
                        Ok(()) => true,
                        // 44 §1.2's `7=無効`, not an internal error: the checkpoint is part of what
                        // was being verified.
                        Err(e) => {
                            return Ok(receipt::anchor_refused(anchor_source, &e.to_string()))
                        }
                    }
                }
                (Some(_), None) => {
                    return Err(Error::Usage {
                        detail: "--checkpoint-key names the key a --checkpoint was signed with, \
                                 and no checkpoint was given. Authenticating an anchor that does \
                                 not exist is not a weaker check, it is no check"
                            .to_string(),
                    })
                }
                (None, _) => false,
            };
            // 🔴 **FR-M7-3**. The list is authenticated against the key being verified with — the
            // only key a third party holds — and the setting is the operator's, which is what
            // 「遡及範囲は政策設定」 means at a command line.
            let ledger = match revocations {
                Some(path) => Some(receipt::read_revocation_ledger(path, &public)?),
                None => None,
            };
            if let Some((_, ignored)) = &ledger {
                if *ignored > 0 {
                    eprintln!(
                        "gx: {ignored} revocation entr(y/ies) name other keys and were not checked \
                         — this verifier holds one public key (FR-M7-3)"
                    );
                }
            }
            let policy = match &ledger {
                Some((ledger, _)) => Some(gx_witness::RevocationPolicy {
                    ledger,
                    retroaction: retroaction_setting(retroaction)?,
                    // 則 2: the verifier's own clock, read in the one place that reads one.
                    verified_at: clock::now(),
                }),
                None => None,
            };
            receipt::verify(
                &source,
                &public,
                anchor.as_ref(),
                anchor_source,
                anchor_authenticated,
                policy.as_ref(),
            )
        }
    }
}

/// `--retroaction`, as the setting `gx_witness` names.
///
/// A refusal rather than a fallback for a word this build does not have: the two settings answer
/// differently about the same receipt, so a misspelling that quietly became the default would give
/// an operator the opposite of what they asked for on exactly the run where it matters.
fn retroaction_setting(word: &str) -> Result<gx_witness::Retroaction> {
    gx_witness::Retroaction::ALL
        .into_iter()
        .find(|setting| setting.as_str() == word)
        .ok_or_else(|| Error::Usage {
            detail: format!(
                "--retroaction takes {:?}; got {word:?}. 「遡及範囲は政策設定」 (req/98 §3-2) and the \
                 two positions are 45 ASM-45-2's default and the one a compromise forces",
                gx_witness::Retroaction::ALL.map(gx_witness::Retroaction::as_str)
            ),
        })
}

fn log_cmd(cli: &Cli, cmd: &LogCmd) -> Result<Outcome> {
    let layout = Layout::open(&project(cli)?)?;
    match cmd {
        LogCmd::Proof { leaf, json: _ } => {
            // Same order as `receipt show`: the argument is parsed before the ledger is opened, so a
            // `--leaf` that is neither an index nor an id is 「入力不正」 rather than 「no ledger」.
            let leaf = ledger::Leaf::parse(leaf)?;
            let store = ledger::open(&layout)?;
            ledger::proof(&store, &leaf)
        }
        LogCmd::Consistency { from, to, json: _ } => {
            let store = ledger::open(&layout)?;
            ledger::consistency(&store, *from, *to)
        }
        LogCmd::Checkpoint { key, origin, out } => {
            let store = ledger::open(&layout)?;
            let path = key.as_ref().ok_or_else(|| Error::Usage {
                detail:
                    "--key names the ledger signing key. §47 M6-24: 「作れるのは台帳の持ち主だけ」\
                         — a checkpoint is a signed statement about this log and nothing else can \
                         stand in for the key"
                        .to_string(),
            })?;
            let pair = gx_witness::KeyPair::load(path)?;
            ledger::checkpoint(&store, &pair, origin, clock::now(), out.as_deref())
        }
    }
}

fn key_cmd(cli: &Cli, cmd: &KeyCmd) -> Result<Outcome> {
    let store = KeyStore::user_default()?;
    match cmd {
        KeyCmd::Gen {
            alg,
            out,
            record,
            json: _,
        } => {
            // 🔴 The project is opened **only** when `--record` asks for it. 44 §1.2's `gen` has no
            // project at all, and a command that opened `.gx/` unasked would fail for an operator
            // making a key outside one — which is most of them (M6H2-6's shape: a verifier's
            // environment is not a project).
            let layout = if *record {
                Some(Layout::open(&project(cli)?)?)
            } else {
                None
            };
            let outcome = keys::gen_recording(&store, alg, out.as_deref(), layout.as_ref())?;
            // Where the secret was filed, on **stderr**: 44 §1.2 fixes stdout to two fields, and
            // `gx key gen --json > pub.json` has to produce exactly those two.
            let filed = out.clone().unwrap_or_else(|| {
                store.path_of(outcome.json["key_id"].as_str().unwrap_or_default())
            });
            eprintln!("gx: secret key written to {} (req/56 §3)", filed.display());
            // 🔴 M6H2-10: `KeyPair::save` asks for 0600 and a filesystem with no unix permission
            // model (drvfs, 9p, a Windows share) silently gives 0777 instead — and `KeyPair::load`
            // then refuses the file this command just wrote. The refusal is right and its timing was
            // not: it arrived at the next command. Said here, where the operator can still choose a
            // different `--out`.
            if let Some(warning) = keys::permission_warning(&filed) {
                eprintln!("gx: {warning}");
            }
            Ok(outcome)
        }
        KeyCmd::List { json: _ } => keys::list(&store),
        KeyCmd::Revoke {
            key_id,
            reason,
            out,
            json: _,
        } => {
            // 則 2: the clock is read in `clock::now` and nowhere else. There is no `--at` here for
            // M6-28's reason — a revocation whose moment is an argument is a boundary an operator
            // can move after the fact, and the boundary is the whole content of the record.
            let outcome = keys::revoke(&store, key_id, reason, clock::now(), None, out.as_deref())?;
            eprintln!(
                "gx: the secret for {key_id:?} is kept — receipts it signed still have to verify \
                 (FR-M7-3)"
            );
            Ok(outcome)
        }
        KeyCmd::Rotate {
            key_id,
            alg,
            reason,
            record,
            json: _,
        } => {
            let layout = if *record {
                Some(Layout::open(&project(cli)?)?)
            } else {
                None
            };
            let outcome = keys::rotate(&store, alg, key_id, reason, clock::now(), layout.as_ref())?;
            if !*record {
                eprintln!(
                    "gx: --record was not given, so `.gx/config.toml` still names the revoked key \
                     if it named one (FR-M7-4)"
                );
            }
            Ok(outcome)
        }
    }
}
