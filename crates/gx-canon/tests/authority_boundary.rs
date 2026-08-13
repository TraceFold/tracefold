// what : 🔴 **則 1 (M6)** — the two secondary surfaces hold no semantic authority. Three counters
//         over `crates/gx-cli/src` and `crates/gx-api/src`:
//           (i)   calls into gx-canon        = 0   (41 §6「全canonical encodeはgx-canon経由のみ」)
//           (ii)  constructions of `Verdict` = 0   (41 §4: judging is the gate's)
//           (iii) writes of a `Lifecycle`    = 0   (42 §1.3-3: 状態は engine 側の外部テーブル)
// why  : req/88 §3 Λ1 makes this M6's central claim rather than a hygiene rule — 「M6 が意味論を
//         足したらそれは実装欠陥であって設計選択ではない」 — and req/88 §6.0-11 says an absence check
//         is empty without a mutation: 「不在の検査は変異なしでは空虚」. `tools/verify_m6h1.sh` §4
//         fires all three and prints which probes fall.
// where: **gx-canon's `tests/`, and neither of the crates it is about.** Two reasons, and the second
//         is the one that chose the file. `unsafe_forbidden.rs` (this same directory) already set the
//         precedent that a repo-wide text gate lives in a crate that is not its own subject — a
//         crate asserting its own source is a crate reading the file it wrote. And `probes/doubt`,
//         where the other membership gates live, is **dropped from `GLOVREX_CI_SCOPE`** on a runner
//         that does not hold the Glovrex_Alpha tree (tools/ci.sh, `fmt_scope`'s narrowed spelling);
//         a gate that vanishes on some machines is 「時々在る gate」, and this is the claim M6 is
//         about. gx-canon is in scope everywhere, and (i) is a claim about gx-canon's monopoly.
// deps : std only. Nothing here compiles the crates it reads — the guard has to fire on a tree that
//         does **not** build too, because 「somebody added a canonical encode to the CLI」 is caught
//         by the text before it is caught by the linker.
// note : M6 hand 1 (req/88 §6.2 手 1 ③). The three names are the ones req/88 §3 Λ1 writes:
//         「`gx-cli`/`gx-api` の `src/` で **(i) `gx_canon::` の呼び出し 0 (ii) `Verdict` を構成する
//         行 0 (iii) `Lifecycle` を書く行 0**」.

use std::path::{Path, PathBuf};

/// The two surfaces this file is about, named rather than globbed, for `SHIPPED_CRATE_ROOTS`'s
/// reason: a third secondary surface added tomorrow should be a decision somebody writes down.
///
/// 🔴 **M6H8-1 採(a), attack A6** (req/38 §55). 「書き留められる事に依存している」 was the whole of the
/// guarantee until this hand: hand 8 added `crates/gx-ext/src/lib.rs`, called gx-canon from it, and
/// all three counters stayed at zero because a third surface is not in a two-name array. The list
/// stays — a decision somebody writes down is still what it is — and it is now **compared against a
/// derivation** ([`derived_secondary_surfaces`]), with the scan walking the union of the two. A
/// surface nobody wrote down is scanned anyway and then reported as an undeclared surface.
const SECONDARY_SURFACES: [&str; 2] = ["crates/gx-cli", "crates/gx-api"];

/// What makes a crate a secondary surface, measured instead of remembered.
///
/// **A surface over the engine is a crate that names the engine**: `gx-cli` and `gx-api` are the two
/// members of this workspace that declare `gx-engine` as a shipping dependency, and every other
/// member is a layer below it (`gx-engine` itself is not a surface *over* itself). That is the
/// predicate 則 1 is actually about — a crate holding the eight entrances is a crate that could mint
/// what it should only be reading — and it is one `cargo`-visible fact rather than a memory.
///
/// The manifest is read as text for `sources`'s reason: nothing here compiles anything, because the
/// guard has to fire on a tree that does not build.
fn derived_secondary_surfaces() -> Vec<String> {
    let root = repo_root();
    let mut out = Vec::new();
    for member in manifest_crate_members() {
        let Ok(text) = std::fs::read_to_string(root.join(&member).join("Cargo.toml")) else {
            continue;
        };
        if declares(&shipping_deps(&text), "gx-engine") {
            out.push(member);
        }
    }
    out.sort();
    out
}

/// The ledger the derivation walks: every workspace member under `crates/`, as the root manifest
/// spells it.
///
/// Split out of [`derived_secondary_surfaces`] by **M7 fix批 ③** so that the denominator is a value
/// something can be compared against, rather than a loop counter nobody can see.
fn manifest_crate_members() -> Vec<String> {
    let root = repo_root();
    let manifest =
        std::fs::read_to_string(root.join("Cargo.toml")).expect("the workspace manifest");
    let members = manifest
        .split("members = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("the workspace declares its members");
    let mut out = Vec::new();
    for line in members.lines() {
        let Some(member) = line.trim().trim_end_matches(',').strip_prefix('"') else {
            continue;
        };
        let member = member.trim_end_matches('"');
        if member.starts_with("crates/") {
            out.push(member.to_string());
        }
    }
    out.sort();
    out
}

/// The members whose manifest the derivation **actually read**.
///
/// 🔴 **M7 fix批 ③, 起票 A-5** (`req/38` §64, `req/105` §4-2). [`derived_secondary_surfaces`] ends a
/// member's turn with `else { continue; }` when its `Cargo.toml` will not open, and that skip left
/// no trace anywhere: a member the derivation never looked at and a member that declares nothing
/// produce the identical answer, and the answer is 「not a secondary surface」. §30's disease, inside
/// the instrument A6 built to cure a neighbouring case of it.
///
/// This function is the same walk with the outcome kept, so
/// [`the_declared_surfaces_are_the_surfaces_this_workspace_has`] can require it to equal
/// [`manifest_crate_members`] — 「歩いた数を印字し、台帳の crates/ member 数と assert する」.
fn walked_crate_members() -> Vec<String> {
    let root = repo_root();
    manifest_crate_members()
        .into_iter()
        .filter(|member| root.join(member).join("Cargo.toml").is_file())
        .filter(|member| std::fs::read_to_string(root.join(member).join("Cargo.toml")).is_ok())
        .collect()
}

/// The surfaces the three counters walk: everything declared, plus anything derived that was not.
fn scanned_surfaces() -> Vec<String> {
    let mut all: Vec<String> = SECONDARY_SURFACES
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    for derived in derived_secondary_surfaces() {
        if !all.contains(&derived) {
            all.push(derived);
        }
    }
    all.sort();
    all
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

/// Every `.rs` file under a crate's `src/`, as `(relative path, source)`.
///
/// A missing `src/` is a **failure**, not an empty scan: three counters that are 0 because the tree
/// they count is absent are the shape §30's ledger is about, and this suite's whole value is that
/// the numbers mean something.
fn sources() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    for surface in scanned_surfaces() {
        let dir = root.join(&surface).join("src");
        assert!(
            dir.is_dir(),
            "{} has no src/ — 則 1's three counters would all be 0 because there is nothing to \
             count, which is a green gate over an absent subject (req/88 §6.2 手 1 creates both \
             crates)",
            dir.display()
        );
        walk(&dir, &root, &mut out);
    }
    assert!(
        !out.is_empty(),
        "the two secondary surfaces hold no .rs file at all"
    );
    out
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("a readable directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            walk(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path
                .strip_prefix(root)
                .expect("under the root")
                .to_string_lossy()
                .replace('\\', "/");
            out.push((
                rel,
                std::fs::read_to_string(&path).expect("a readable source file"),
            ));
        }
    }
}

/// The source with `//`-comments removed, line by line.
///
/// The M5 lesson §30 recorded twice: 「無い事」の grep は comment を除外し invocation 行のみ. Both
/// crates discuss `gx_canon` and `Verdict` at length in their documentation — that discussion is
/// *why* the rule holds — and a scanner that read prose would report the explanation as the
/// violation. Only `//` is handled: neither crate uses `/* */`, and a stripper that tried to would
/// have to lex Rust.
///
/// # 🔴 **M6H8-1 採(a), attack A2** — a `//` inside a string is not a comment
///
/// The first spelling cut every line at its first `//`, and 44 §2.3 defines `type` as a URI, so
/// **the specification requires the shipped code to contain `https://`** — two lines of it today
/// (`crates/gx-cli/src/lib.rs` and `crates/gx-api/src/gx_code.rs`), and one more with every erratum
/// that adds a `gx_code`. Everything to the right of such a URL was invisible to all three counters,
/// which made `let _u = "https://…"; let _ = gx_canon::cid::compute(t);` a violation nobody saw. The
/// worst part of it is that it came from a **correct** discipline correctly implemented: §30 asked
/// for comments to be excluded, and this is what excluding them looked like.
///
/// The fix is quote parity rather than a lexer — the cheapest thing that is *right about the shape
/// that occurs*: a `"` that is not escaped opens and closes a string, a `'…'` char literal is
/// stepped over whole (so that `'"'` does not unbalance the line), and a `//` counts only outside a
/// string. Raw strings (`r#"…"#`) would need the hash count carried; neither surface holds one
/// today, and if one arrives it can only make this stricter (the rest of the line stays scanned).
fn code_only(source: &str) -> String {
    source
        .lines()
        .map(code_of_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// One line, up to the first `//` that is **not** inside a string or char literal.
fn code_of_line(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    let mut in_string = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_string => i += 1, // the escaped byte is never a delimiter
            b'"' => in_string = !in_string,
            b'\'' if !in_string => {
                // A char literal: `'a'`, `'\''`, `'"'`. A lifetime (`'a` with no closing quote) is
                // left alone by stepping only when a close is actually there.
                let rest = &bytes[i + 1..];
                let width = match rest.first() {
                    Some(b'\\') => 3, // \x' -- backslash, one escape byte, quote
                    Some(_) => 2,     // c'
                    None => 0,
                };
                if width > 0 && rest.get(width - 1) == Some(&b'\'') {
                    i += width;
                }
            }
            b'/' if !in_string && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// (i) Lines that reach into gx-canon.
///
/// Both spellings, because either one is the迂回 41 §6 forbids: a `use gx_canon::…` import and a
/// `gx_canon::…` path expression.
fn canon_reaches(code: &str) -> usize {
    code.lines()
        .filter(|l| l.contains("gx_canon::") || l.contains("use gx_canon"))
        .count()
}

/// (ii) Lines that construct a `Verdict`.
///
/// The three variants by name (41 §3: `Admit(AdmitProof) | Deny(Vec<Reason>) | Escalate(Ticket)`).
/// **Not** every mention of the word: a CLI that prints a verdict the engine handed it is doing
/// exactly what 則 1 permits, and `VerdictKind` — the display-only discriminant gx-core owns — is a
/// different type. What is forbidden is *minting* one.
fn verdict_constructions(code: &str) -> usize {
    code.lines()
        .filter(|l| {
            l.contains("Verdict::Admit")
                || l.contains("Verdict::Deny")
                || l.contains("Verdict::Escalate")
        })
        .count()
}

/// (iii) Lines that write a lifecycle.
///
/// Two shapes, because 42 §1.3-3's claim (「状態は`TransformationId`をキーとしたengine側の外部
/// テーブルで管理される」) is broken in two different ways and only one of them is a constructor:
///
/// * a **field** whose declared type mentions `Lifecycle` — the CLI keeping its own state table;
/// * a `Lifecycle::<Variant>` on the **right of an `=`** or inside a struct literal — the CLI
///   minting a state value.
///
/// A `match` arm over a lifecycle the engine returned matches neither, which is the point: reading
/// a state is what the read-only surface is for, and hand 2 onwards will do it on every line of
/// output.
fn lifecycle_writes(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in code.lines() {
        let t = line.trim();
        // A struct field declaration: `name: <type mentioning Lifecycle>,`
        let field = t.ends_with(',')
            && t.contains(": ")
            && t.contains("Lifecycle")
            && !t.contains("=>")
            && !t.starts_with("//");
        // A construction in value position.
        let minted = match t.find("Lifecycle::") {
            Some(at) => {
                let before = &t[..at];
                before.contains('=') && !before.contains("=>") || before.trim_end().ends_with(':')
            }
            None => false,
        };
        if field || minted {
            out.push(t.to_string());
        }
    }
    out
}

/// Whether a `[dependencies]` body declares `crate`, **under any name**.
///
/// # 🔴 **M6H8-1 採(a), attack A1** — `package =` is the quietest door
///
/// The first spelling asked whether a dependency line **started with** `gx-canon`, and cargo's
/// rename form does not:
///
/// ```text
/// canon = { package = "gx-canon", path = "../gx-canon", version = "0.1.0" }
/// ```
///
/// That single line takes out **both halves** of 則 1 (i) at once — the manifest half stops seeing a
/// declaration, and the source half never saw `canon::cid::compute(t)` because it hunts the literal
/// `gx_canon::`. req/88 called the manifest half 「the stronger statement」 on the grounds that a
/// crate which cannot name gx-canon cannot call it; under a rename the crate names it by another
/// word, and the argument stops holding. So both spellings are read: the key, and `package =`.
fn declares(deps: &str, crate_name: &str) -> bool {
    let renamed = format!("package = \"{crate_name}\"");
    let renamed_tight = format!("package=\"{crate_name}\"");
    deps.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .any(|l| {
            let key = l.split(['=', ' ', '.']).next().unwrap_or_default();
            key == crate_name || l.contains(&renamed) || l.contains(&renamed_tight)
        })
}

/// The `[dependencies]` body of a manifest, up to the next section header.
///
/// # 🔴 The three spellings this reader does not read (**M7 fix批 ③, 起票 A-5**)
///
/// `req/105` §4-2 measured the reader rather than its answer: it finds the **first** `\n[dependencies]`
/// and stops at the next `[`, so a `gx-engine` dependency written any of these three ways is
/// **absent** as far as [`derived_secondary_surfaces`] is concerned, and a surface carrying one
/// would not be walked by 則 1's three counters:
///
/// | spelling | count on this tree (M7) |
/// |---|---|
/// | `[target.'cfg(…)'.dependencies]` naming `gx-engine` | **0** — one `[target…]` section exists at all (`crates/gx-witness/Cargo.toml`, `getrandom`), and it names no engine |
/// | `[dependencies.gx-engine]`, the table form | **0** |
/// | a **second** `[dependencies]` section | **0** |
///
/// The counts are today's, and they are why this is a disclosure rather than a defect: the reader is
/// correct about every manifest in this workspace. It is written down because 「A6 が塞いだ穴の再来」
/// is what a fourth spelling would be — a secondary surface gaining the engine without the counters
/// noticing — and because the alternative (a full TOML parse in a suite whose whole premise is that
/// it runs on a tree that does **not** build) buys the coverage at the cost of the property that
/// makes the gate fire early. The limit stated is worth more than the limit hidden: this is the same
/// choice `the_text_gate_declares_its_own_limits` makes about A3/A4.
fn shipping_deps(manifest: &str) -> String {
    let Some(start) = manifest.find("\n[dependencies]") else {
        return String::new();
    };
    let rest = &manifest[start + "\n[dependencies]".len()..];
    let end = rest
        .match_indices('\n')
        .find(|(i, _)| rest[i + 1..].starts_with('['))
        .map_or(rest.len(), |(i, _)| i);
    rest[..end].to_string()
}

// ---------------------------------------------------------------------------
// The three counters
// ---------------------------------------------------------------------------

/// 🔴 **則 1 (i)** — neither secondary surface reaches into gx-canon, in source or in manifest.
///
/// Two halves and both are needed. The manifest half is the stronger statement (a crate that cannot
/// name gx-canon cannot call it) and the source half is the one that catches the intermediate state
/// where the dependency is declared 「for later」 — 41 §6's monopoly is about calls, and a call is
/// written before it is reviewed.
#[test]
fn no_secondary_surface_encodes_canonically() {
    let files = sources();
    let mut offences: Vec<String> = Vec::new();
    let mut lines = 0usize;
    for (rel, source) in &files {
        let code = code_only(source);
        lines += code.lines().count();
        let n = canon_reaches(&code);
        if n > 0 {
            offences.push(format!("{rel}:{n}"));
        }
    }

    let root = repo_root();
    let mut declaring: Vec<String> = Vec::new();
    for surface in scanned_surfaces() {
        let manifest = std::fs::read_to_string(root.join(&surface).join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("{surface}/Cargo.toml is unreadable: {e}"));
        if declares(&shipping_deps(&manifest), "gx-canon") {
            declaring.push(surface);
        }
    }

    println!(
        "SECONDARY_SRC_FILES={} SECONDARY_CODE_LINES={lines} CANON_CALLS={} CANON_DEPENDENCIES={}",
        files.len(),
        offences.len(),
        declaring.len()
    );
    assert!(
        offences.is_empty(),
        "則 1 (i): a secondary surface reaches into gx-canon ({offences:?}). 41 §6 gives the \
         canonical encode one door; a CLI that mints a `Cid` is a CLI that can name a \
         transformation the engine never saw"
    );
    assert!(
        declaring.is_empty(),
        "則 1 (i): {declaring:?} declare gx-canon as a shipping dependency"
    );
}

/// 🔴 **則 1 (ii)** — neither secondary surface constructs a `Verdict`.
#[test]
fn no_secondary_surface_constructs_a_verdict() {
    let files = sources();
    let mut offences: Vec<String> = Vec::new();
    for (rel, source) in &files {
        let n = verdict_constructions(&code_only(source));
        if n > 0 {
            offences.push(format!("{rel}:{n}"));
        }
    }
    println!("VERDICT_CONSTRUCTIONS={}", offences.len());
    assert!(
        offences.is_empty(),
        "則 1 (ii): a secondary surface builds a `Verdict` ({offences:?}). 41 §4 puts the one \
         judgement in `Gate::verify`; an API handler that answers `Admit` is a second gate with no \
         policy behind it"
    );
}

/// 🔴 **則 1 (iii)** — neither secondary surface writes a `Lifecycle`.
#[test]
fn no_secondary_surface_writes_a_lifecycle() {
    let files = sources();
    let mut offences: Vec<String> = Vec::new();
    for (rel, source) in &files {
        for line in lifecycle_writes(&code_only(source)) {
            offences.push(format!("{rel}: {line}"));
        }
    }
    println!("LIFECYCLE_WRITES={}", offences.len());
    assert!(
        offences.is_empty(),
        "則 1 (iii): a secondary surface holds or mints a lifecycle ({offences:?}). 42 §1.3-3 puts \
         the state table on the engine side; a CLI that keeps one has a second answer to 「what \
         state is this in」 and req/88 Λ2's observational equivalence stops holding"
    );
}

// ---------------------------------------------------------------------------
// The instrument, doubted
// ---------------------------------------------------------------------------

/// The three scanners see what they are looking for.
///
/// §30's disease, prevented rather than diagnosed: an absence probe whose scanner is broken reports
/// 0 forever and reads exactly like a passing gate. So each counter is handed a synthetic line
/// carrying the shape it hunts, and has to find it. This is the same positive control
/// `tools/verify_m6h1.sh` §4 fires against the real tree — one in-process, one on disk — and the two
/// disagree only if the mutation stopped applying.
#[test]
fn the_scanners_see_the_shapes_they_hunt() {
    assert_eq!(canon_reaches("let c = gx_canon::cid::compute(&t)?;"), 1);
    assert_eq!(canon_reaches("use gx_canon::cid;"), 1);
    assert_eq!(
        canon_reaches(&code_only("// gx_canon:: is not called here")),
        0,
        "the comment stripper runs first -- and it is `code_only` that runs it, not `canon_reaches`. \
         This assertion is written with the pipeline the real scan uses, because a control that \
         exercised a different pipeline would be a control over nothing (it caught exactly that \
         here: the first spelling asserted 0 against the raw string and was red on its own file)"
    );
    assert_eq!(verdict_constructions("let v = Verdict::Deny(vec![]);"), 1);
    assert_eq!(
        verdict_constructions("Verdict::Admit(proof) => println!(),"),
        1
    );
    assert_eq!(
        lifecycle_writes("    state: Lifecycle,").len(),
        1,
        "a state field is a state table"
    );
    assert_eq!(
        lifecycle_writes("let s = Lifecycle::Committed;").len(),
        1,
        "minting a state in value position"
    );
    assert_eq!(
        lifecycle_writes("        Lifecycle::Committed => \"committed\",").len(),
        0,
        "reading a state the engine handed over is what the read-only surface is for"
    );
    println!("SCANNER_POSITIVE_CONTROLS=8");
}

/// 🔴 **A2 closed**: a `//` inside a string literal does not blind the scanners.
///
/// The negative control is the shape 44 §2.3 requires — a `type` URI — with a call after it on the
/// same line. Hand 8 measured this as a violation the instrument could not see (`RC=0` on a tree
/// that reached into gx-canon), and the two shipped lines that were being truncated are asserted
/// here as well: the fix is worth a probe precisely because the specification keeps producing the
/// input that broke it.
#[test]
fn a_comment_marker_inside_a_string_is_not_a_comment() {
    let attack = r#"let _u = "https://glovrex.dev/errors/x"; let _ = gx_canon::cid::compute(t);"#;
    assert_eq!(
        canon_reaches(&code_only(attack)),
        1,
        "M6H8-1 A2: the URL's `//` used to end the line for every counter"
    );
    assert_eq!(
        verdict_constructions(&code_only(
            r#"let _u = "http://x"; let _v = Verdict::Deny(vec![]);"#
        )),
        1
    );
    assert_eq!(
        lifecycle_writes(&code_only(
            r#"let _u = "//"; let s = Lifecycle::Committed;"#
        ))
        .len(),
        1
    );
    // Still a comment when it is one, still a comment after a closed string, and a char literal
    // holding a quote does not unbalance the line.
    assert_eq!(canon_reaches(&code_only("// gx_canon::cid")), 0);
    assert_eq!(
        canon_reaches(&code_only(r#"let _u = "x"; // gx_canon::cid"#)),
        0
    );
    assert_eq!(
        canon_reaches(&code_only(r#"let q = '"'; // gx_canon::"#)),
        0
    );
    assert_eq!(
        canon_reaches(&code_only(
            r#"let q = '"'; let _ = gx_canon::cid::compute(t);"#
        )),
        1
    );

    // The lines 44's own error-type URIs put in the shipped surfaces. Their content has nothing to
    // do with 則 1 — what matters is that they exist (so this probe has a subject) and that the
    // scanner now reads past them. Comment lines are excluded because a comment *should* be cut at
    // its first `//`, which is the whole of §30's rule.
    let root = repo_root();
    let mut carrying = 0usize;
    let mut truncated = 0usize;
    for (rel, _) in sources() {
        let source = std::fs::read_to_string(root.join(&rel)).expect("readable");
        for line in source.lines() {
            if !line.contains("://") || line.trim_start().starts_with("//") {
                continue;
            }
            carrying += 1;
            if code_of_line(line).len() < line.trim_end().len() {
                truncated += 1;
            }
        }
    }
    println!("URL_LINES_IN_CODE={carrying} URL_LINES_STILL_TRUNCATED={truncated}");
    assert!(
        carrying >= 2,
        "hand 8 measured two shipped lines whose string holds a `//` (44 §2.3 defines `type` as a \
         URI); finding none means this probe has no subject and is measuring nothing"
    );
    assert_eq!(
        truncated, 0,
        "a line carrying a URL is still being cut at the URL's own slashes"
    );
}

/// 🔴 **A1 closed**: a renamed dependency is still a declared dependency.
#[test]
fn a_renamed_dependency_is_seen() {
    assert!(
        declares(
            "canon = { package = \"gx-canon\", path = \"../gx-canon\", version = \"0.1.0\" }",
            "gx-canon"
        ),
        "M6H8-1 A1: cargo's rename form took out both halves of 則 1 (i) at once"
    );
    assert!(declares(
        "gx-canon = { path = \"../gx-canon\" }",
        "gx-canon"
    ));
    assert!(declares("gx-canon.workspace = true", "gx-canon"));
    assert!(!declares(
        "# gx-canon = { path = \"../gx-canon\" }",
        "gx-canon"
    ));
    assert!(!declares(
        "gx-canonical = { path = \"../other\" }",
        "gx-canon"
    ));
    assert!(!declares("serde = \"1\"", "gx-canon"));
    println!("RENAME_CONTROLS=6");
}

/// 🔴 **A6 closed**: the set of secondary surfaces is measured, and the written list has to agree.
///
/// Hand 8 added a third surface and the instrument did not exist as far as it was concerned. The
/// derivation is 「a workspace member under `crates/` that declares `gx-engine`」, which is what a
/// surface over the engine is; the literal list stays because 「a third secondary surface should be a
/// decision somebody writes down」 stays true. What changed is that failing to write it down is now
/// **red** rather than silent, and the counters walk the union either way.
///
/// # 🔴 And the denominator, which A6 did not have (**M7 fix批 ③, 起票 A-5**)
///
/// `req/105` §4-2: 「歩いた数を印字する行が導出の側に無いので、飛ばされた事は log に残らない」. A6 made
/// the *set* derived and left the *walk* unmeasured, so a member whose `Cargo.toml` cannot be opened
/// was skipped by `else { continue; }` and arrived at this assertion looking exactly like a member
/// that declares nothing. The audit hand measured the walk from outside, in
/// `tools/verify_m7audit.sh`, and then refused to leave it there — 「それは監査手の計器であって repo の
/// gate ではない」 — because an instrument that lives in a lane's script stops running when the lane
/// ends. So the count is asserted here, where `cargo test` reaches it: the members the derivation
/// read have to be **every** member the ledger declares under `crates/`.
#[test]
fn the_declared_surfaces_are_the_surfaces_this_workspace_has() {
    let ledger = manifest_crate_members();
    let walked = walked_crate_members();
    println!(
        "A6_CRATE_MEMBERS_DECLARED={} A6_MEMBERS_WALKED_BY_DERIVATION={}",
        ledger.len(),
        walked.len()
    );
    assert_eq!(
        walked,
        ledger,
        "the derivation walked {} of the {} members `Cargo.toml` declares under `crates/`. The \
         missing ones were skipped because their manifest would not open, and a skipped member is \
         indistinguishable from a member that declares no engine — which is the shape that let a \
         third secondary surface exist unseen in the first place (M6H8-1 攻撃 A6)",
        walked.len(),
        ledger.len()
    );

    let derived = derived_secondary_surfaces();
    let mut declared: Vec<String> = SECONDARY_SURFACES
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    declared.sort();
    println!("DERIVED_SECONDARY_SURFACES={derived:?} DECLARED={declared:?}");
    assert_eq!(
        derived, declared,
        "the crates that declare gx-engine are not the crates named in SECONDARY_SURFACES. A \
         surface that appeared without being written down is scanned by the three counters anyway \
         (`scanned_surfaces`), and this assertion is where the decision gets recorded — add it to \
         the array with the reason, or take the engine dependency back out"
    );
    assert!(
        !derived.is_empty(),
        "no member declares gx-engine: the derivation is measuring nothing, which is §30's disease \
         in the instrument that exists to prevent it"
    );
}

/// 🔴 **A3 / A4 are not closed, and this is where that is written down.**
///
/// req/38 §55 (M6H8-1 採(a)) rules the alias attacks **out of scope on purpose**: `use gx_core::
/// Verdict as V; let _v = V::Deny(vec![]);` and `type H8St = gx_core::Lifecycle; state: H8St,` are
/// invisible to any counter that reads text, because resolving an alias needs the type information
/// a compiler has and a scanner does not. 「閉じたふりをする lexer より安全」 — a scanner that appeared
/// to handle aliases would be the more dangerous instrument, since a reader would stop looking.
///
/// What holds the line instead is review and the compiler-side facts 則 1 rests on: gx-canon is not
/// a dependency of either surface at all (so no alias can reach it), and `Verdict`/`Lifecycle`
/// aliases are visible in a diff as new `type` items. This probe asserts the **first** of those,
/// which is mechanical, and states the second as the limit it is.
#[test]
fn the_text_gate_declares_its_own_limits() {
    const LIMITS: [&str; 2] = [
        "A3: `use gx_core::Verdict as V; V::Deny(..)` -- an aliased constructor",
        "A4: `type S = gx_core::Lifecycle; state: S,` -- an aliased field type",
    ];
    println!("TEXT_GATE_UNCLOSED_ATTACKS={LIMITS:?}");

    // The compiler-side half of 則 1 (i): no alias can call a crate the manifest does not name.
    let root = repo_root();
    for surface in scanned_surfaces() {
        let manifest = std::fs::read_to_string(root.join(&surface).join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("{surface}/Cargo.toml is unreadable: {e}"));
        assert!(
            !declares(&shipping_deps(&manifest), "gx-canon"),
            "{surface} names gx-canon: the alias limit above stops being bounded by the manifest"
        );
    }
    assert_eq!(LIMITS.len(), 2, "two attacks stay open, deliberately");
}
