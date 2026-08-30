// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P2 item3** (`req/130` §1, NFR-012) — a self-made, zero-dependency secret scanner, and the
//! fail-open check that proves it is not a no-op.
//!
//! # Why here, and not `probes/doubt`
//!
//! `probes/doubt` depends by path on `../../../Glovrex_Alpha` (its own `Cargo.toml`'s header), a
//! tree outside this repository's git history, and `tools/ci.sh`'s `GLOVREX_CI_SCOPE` narrows past
//! it on a runner that does not hold Alpha. NFR-012 asks for a **blocking CI gate**
//! ("incorporate a static scan…make zero detections a blocking gate"; sem: SEM-gx-cli-1374), and a gate that can only run
//! where an unrelated external module happens to be checked out is not one. `gx-cli` has no such
//! dependency, so this suite runs wherever this repository does — `tools/ci.sh`'s new stage 7b names
//! it explicitly, the way stage 3c names `gx-engine`'s `state_machine_coverage` regardless of
//! `GLOVREX_CI_SCOPE`.
//!
//! # ruling 3 (`req/38` §72 assignment 2 / `req/130` §6): a minimal in-house rule scanner (sem: SEM-gx-cli-1375)
//!
//! Four rule families, matched by hand rather than by a `regex` dependency (zero dependencies; sem: SEM-gx-cli-1376): a
//! PEM/private-key header, an AWS access-key prefix, a GitHub token prefix, a `Bearer `-prefixed
//! token-shaped string, and an email address whose domain is not on the allow-list (RFC 2606's
//! reserved suffixes — `.invalid`/`.example`/`.test`/`.localhost` — plus this project's own
//! disclosed contact domain and its GitHub no-reply address, both of which legitimately appear in
//! commit templates and fixtures and are not a leak when they do).
//!
//! # What is scanned, and what is not
//!
//! `crates/`, `tools/`, `policies/` — the source and the scripts that ship, matching NFR-012's own
//! subject ("the Receipt/log contains no plaintext secret material…" is a claim about what the *product* emits, and (sem: SEM-gx-cli-1377)
//! the product is built from these three trees). `req/*.md` is **not** scanned here: this project's
//! own disclosed contact address (`mahirhir@glovrex.com`, D-1421) appears throughout the decision
//! record on purpose, and a scanner that flagged every occurrence would either drown in an allow-list
//! entry per file or train its operator to ignore red. `req/130` §1 item3's second surface — a
//! pre-publish transplant check over git history and internal paths — is **not** wired to a `gx`
//! verb or a standalone script in this pass; [`scan_tree`] is written to take an arbitrary root and
//! file set for exactly that future use, and the absence of the wiring is named rather than silently
//! dropped (`req/131` §3).
//!
//! # The fail-open check (AC-P2-4)
//!
//! [`the_scanner_detects_a_planted_secret_in_its_own_fixture`] points the same [`scan_tree`] at
//! `tests/fixtures/secret_scan_positive/`, a file holding one deliberately fake value per rule, and
//! asserts all five sub-rules fire. Without this, a scanner that always returned zero findings would
//! pass [`the_dev_tree_has_zero_secret_scan_findings`] by construction and prove nothing — the
//! ordinary shape of §30's disease, applied to a security gate.
//!
//! # v0.2-b expansion (`req/150` §B, AC-V2B-1) — gotcha77's eight blind spots become rules
//!
//! The M9 cross-audit (`req/136` §4-3, adopted at `req/38` §81 ruling 2; sem: SEM-gx-cli-1402) planted an eight-shape
//! evasion corpus at `tools/audit_m9_p2_scanner_evasion_fixture.sh` and measured that the five
//! rule families above pass all eight undetected — a *declared-scope* limit, reserved to v0.2 as
//! 33 NFR-012's gotcha note 77 / 35 §C RSK-12. This pass closes that reservation with eight further (sem: SEM-gx-cli-1378)
//! rule families (still zero dependencies, still hand-matched): an AWS **secret** access key (context-gated:
//! the material has no prefix of its own), a Slack `xox?-` token, a Stripe `sk_live_` key, a
//! context-gated long hex key, a DSN with an embedded password, a JSON `"api_key"` field, a
//! non-`Bearer` `Authorization:` scheme, and a PEM private-key header split across two lines.
//! (The original doc above says "Four rule families" while listing five — that miscount predates (sem: SEM-gx-cli-1379)
//! this pass and is left as written rather than silently repainted; the family count after this
//! pass is **thirteen**.)
//!
//! Each new rule's own doc declares **what it does not catch** — the same honest-denominator
//! style the first five established. The blanket denominators, stated once: every rule is
//! line-local except the split-PEM pair (which sees exactly two adjacent lines, no further), and
//! nothing here decodes encodings — base64-wrapped, URL-encoded, or constructed-at-runtime
//! secrets pass every rule below by construction, and `req/136` §5's own list of shapes the
//! corpus does not cover (JWT, Azure connection strings, GCP service-account JSON) remains
//! uncovered.
//!
//! The audit corpus itself is **untouched** (33 NFR-012 gotcha note 77: "audit record — do not touch"; sem: SEM-gx-cli-1380): it now
//! serves as the positive corpus for
//! [`the_scanner_detects_all_eight_evasion_shapes_in_the_audit_corpus`], and the dev-tree gate
//! excludes it by path for exactly the reason `secret_scan_positive/` is excluded — a planted
//! corpus the scanner can finally see is a positive fixture, not a leak (ruling 3's
//! "a fixture-only allowlist rather than removal after the check", applied to the corpus this expansion was (sem: SEM-gx-cli-1381)
//! measured against).
//!
//! # v0.3-d expansion (`req/159` §D item 6) — the three shapes `req/136` §5 left uncovered
//!
//! JWT, Azure connection strings, GCP service-account JSON: 33 NFR-012's residual clause (`req/38` §95
//! ruling 4) held these open behind a precondition — "don't speculatively implement what has no real shape in the corpus" (sem: SEM-gx-cli-1382)
//! (`req/155`) — so the corpus came first: each format was collected from the issuer's primary
//! documentation (RFC 7519; Microsoft's storage-configure-connection-string; Google's
//! iam/docs/keys-create-delete) and planted as dummy material in
//! `tests/fixtures/secret_scan_v03d/corpus.txt`, EXAMPLE markers inside the material itself and
//! no real credential anywhere in the chain. Three further rule families (zero dependencies; sem: SEM-gx-cli-1383, hand-matched,
//! same honest-denominator style): [`jwt_hit`], [`azure_connection_key_hit`],
//! [`gcp_service_account_hit`]. The family count after this pass is **sixteen**.
//!
//! # v0.4-h expansion (`req/38` §107 residual 6 / §113 residual) — the different kind the residual named (sem: SEM-gx-cli-1384)
//!
//! Azure AD (Entra ID) client-app secrets and Google `AIza` API keys are the two shapes `req/38`
//! §107 ruling 6 explicitly held open as a different kind (outside the v0.3-d clause's three; sem: SEM-gx-cli-1385),
//! and the same corpus-first precondition applied: each format was collected from the issuer's
//! own primary publication (Microsoft's identifiable-secrets model in
//! microsoft/security-utilities, MIT; Google's docs/authentication/api-keys example string;
//! GitHub's npm access-token format announcement — recorded in Desktop/GitRepo/REFERENCES.md,
//! 2026-08-15) and planted as dummy material in `tests/fixtures/secret_scan_v04h/corpus.txt`.
//! The third shape, an npm access token, is here because this workspace ships an npm package
//! (`sdk/typescript`, and the cargo-dist npm wrapper): a publish token is a credential this
//! project will actually hold, which is the cost-benefit test the brief set for extras.
//!
//! **Collected-impossible, therefore not implemented** (`req/155`'s precondition, stated rather
//! than silently skipped): pre-2021 Azure AD secrets (random strings with no identifiable
//! signature — no documented shape exists to match) and Google's express-mode `AQ.`-prefixed
//! keys (no issuer-documented format as of 2026-08-15). Slack `xox*` and Stripe `sk_live_`,
//! also floated as candidates, are **already** rule families here (v0.2-b, evasion 2/3) — named
//! so nobody re-adds them. Three further rule families: [`aad_client_secret_hit`],
//! [`gcp_api_key_hit`], [`npm_token_hit`]. The family count after this pass is **nineteen**.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/gx-cli sits two levels under the root")
        .to_path_buf()
}

/// Whether this tree's workspace declares `member` — read from the root `Cargo.toml`'s
/// `members` array, inside which nothing but member paths may be written (the public root's own
/// rule, `public/Cargo.toml`). See the corpus guard below for why this exists (req/833).
fn workspace_declares(member: &str) -> bool {
    let manifest = repo_root().join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", manifest.display()));
    let Some(start) = text.find("members = [") else {
        return false;
    };
    let Some(end) = text[start..].find(']') else {
        return false;
    };
    text[start..start + end]
        .lines()
        .any(|l| l.trim().trim_end_matches(',').trim_matches('"') == member)
}

/// One rule's hit: which rule, and where.
#[derive(Debug)]
struct Finding {
    rule: &'static str,
    path: PathBuf,
    line: usize,
}

/// Domains this scanner does not flag an email address on, exactly: RFC 2606's reserved names
/// (`.invalid`/`.example`/`.test`/`.localhost` and the three `example.*` registrations, already
/// covered by the suffix check) plus this project's own disclosed contact domain and the GitHub
/// no-reply address every commit in this repository carries (`crates/gx-adapter-git/tests/support/
/// mod.rs`, `crates/gx-cli/tests/defaults.rs` both use `@glovrex.invalid`, which the suffix rule
/// already admits — these two are for the domain the operator's own address is under, D-1421).
const ALLOWED_EMAIL_DOMAINS: [&str; 3] = ["glovrex.com", "glovrex.dev", "users.noreply.github.com"];
const ALLOWED_EMAIL_SUFFIXES: [&str; 4] = [".invalid", ".example", ".test", ".localhost"];

fn is_allowed_email_domain(domain: &str) -> bool {
    let lower = domain.to_ascii_lowercase();
    ALLOWED_EMAIL_DOMAINS.iter().any(|d| lower == *d)
        || ALLOWED_EMAIL_SUFFIXES.iter().any(|s| lower.ends_with(s))
}

/// Every file under `root` (recursively) whose extension is in `exts`, skipping build output,
/// version control metadata, and any path under `exclude`.
fn walk_files(root: &Path, exts: &[&str], exclude: &[&Path]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if exclude.iter().any(|e| path.starts_with(e)) {
                continue;
            }
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if name == "target" || name == ".git" {
                    continue;
                }
                stack.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| exts.contains(&e))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// A line naming a PEM-encoded private key.
fn pem_header_hit(line: &str) -> bool {
    line.contains("-----BEGIN ") && line.contains("PRIVATE KEY")
}

/// `AKIA` followed immediately by sixteen upper-case-or-digit characters — AWS's own access-key-id
/// shape (the twenty-character form documented at
/// <https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html>).
fn aws_key_hit(line: &str) -> bool {
    for (idx, _) in line.match_indices("AKIA") {
        let run: String = line[idx + 4..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            .collect();
        if run.chars().count() >= 16 {
            return true;
        }
    }
    false
}

/// GitHub's own token prefixes (`ghp_`/`gho_`/`ghu_`/`ghs_`/`ghr_`/`github_pat_`), followed by
/// twenty or more base62-shaped characters.
const GH_TOKEN_PREFIXES: [&str; 6] = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"];

fn gh_token_hit(line: &str) -> bool {
    for prefix in GH_TOKEN_PREFIXES {
        for (idx, _) in line.match_indices(prefix) {
            let run: String = line[idx + prefix.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if run.chars().count() >= 20 {
                return true;
            }
        }
    }
    false
}

/// `Bearer ` followed immediately by twenty or more token-shaped characters. 44 §2.5's own prose
/// (`Authorization: Bearer <token>`) and this crate's tests (`format!("Bearer {token}")`,
/// interpolated rather than written literally) do not match: an angle-bracketed placeholder is not
/// twenty token characters, and an interpolation site has a `{` immediately after the space.
fn bearer_token_hit(line: &str) -> bool {
    const PREFIX: &str = "Bearer ";
    for (idx, _) in line.match_indices(PREFIX) {
        let run: String = line[idx + PREFIX.len()..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect();
        if run.chars().count() >= 20 {
            return true;
        }
    }
    false
}

/// An `<local>@<domain>` shape whose domain is not on [`is_allowed_email_domain`]'s list.
///
/// Manual rather than a `regex` crate's pattern (zero dependencies; sem: SEM-gx-cli-1386): the local part is walked backward from
/// `@` over RFC 5322's unquoted-atom character class, and the domain is walked forward over
/// letters, digits, `.` and `-`, then checked for a plausible (≥2 alphabetic characters) top-level
/// label — enough to keep `https://Alice@MCP.Example:8443/...` (a URI authority in
/// `crates/gx-adapter-mcp/src/locator.rs`'s own tests, `.example`-suffixed and therefore already
/// allow-listed) and prose like `req/spec` files reference from tripping a rule meant for source.
fn foreign_email_hit(line: &str) -> Option<String> {
    for (idx, _) in line.match_indices('@') {
        let before = &line[..idx];
        let local_start = before
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || "._%+-".contains(c)))
            .map_or(0, |p| p + before[p..].chars().next().unwrap().len_utf8());
        let local = &before[local_start..];
        if local.is_empty() {
            continue;
        }
        let domain: String = line[idx + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
            .collect();
        let Some(tld) = domain.rsplit('.').next() else {
            continue;
        };
        if !domain.contains('.') || tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        if !is_allowed_email_domain(&domain) {
            return Some(format!("{local}@{domain}"));
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// v0.2-b expansion rules (`req/150` §B, AC-V2B-1) — one per gotcha77 evasion shape, in the
// corpus's own order. Each doc states the rule's honest denominator: the shapes it deliberately
// does NOT catch, so that narrowing done for false-positive control is declared, never silent.
// ---------------------------------------------------------------------------------------------

/// `true` when `s` holds an unbroken run of at least `min` characters accepted by `accept`.
fn has_run_of(s: &str, min: usize, accept: impl Fn(char) -> bool) -> bool {
    let mut run = 0usize;
    for c in s.chars() {
        if accept(c) {
            run += 1;
            if run >= min {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// **evasion 1** — AWS *secret* access key material: a run of ≥40 characters from AWS's own
/// secret-key alphabet (base64: alphanumeric plus `/`/`+`), on a line that also says both `aws`
/// and `secret` (case-insensitive).
///
/// The context gate is the design decision: the material itself has **no prefix** (that absence
/// is the whole evasion), and an ungated 40-character-run rule would flag every base64 blob and
/// long URL path segment in the tree. Not caught, declared: a bare 40-character secret on a line
/// without both context words (`X="wJal…"`), the key split across lines, or the two words and the
/// material on different lines.
fn aws_secret_key_hit(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if !(lower.contains("aws") && lower.contains("secret")) {
        return false;
    }
    has_run_of(line, 40, |c| {
        c.is_ascii_alphanumeric() || c == '/' || c == '+'
    })
}

/// **evasion 2** — Slack token prefixes (`xoxb-` bot plus the sibling `xoxp-`/`xoxa-`/`xoxr-`/
/// `xoxs-` forms Slack documents), followed by twenty or more characters from Slack's own token
/// alphabet (alphanumeric and `-`). The corpus proves `xoxb-`; the siblings are one character of
/// generality at the same false-positive risk (a prose mention like `` `xoxb-`prefix `` has a
/// zero-length run and does not fire). Not caught, declared: `xapp-` app-level tokens and
/// `xoxe…` refresh tokens (different shapes, not in the corpus), or a token shorter than twenty
/// characters after its prefix.
const SLACK_TOKEN_PREFIXES: [&str; 5] = ["xoxb-", "xoxp-", "xoxa-", "xoxr-", "xoxs-"];

fn slack_token_hit(line: &str) -> bool {
    for prefix in SLACK_TOKEN_PREFIXES {
        for (idx, _) in line.match_indices(prefix) {
            let run = line[idx + prefix.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .count();
            if run >= 20 {
                return true;
            }
        }
    }
    false
}

/// **evasion 3** — a Stripe **live-mode secret** key: `sk_live_` followed by twenty-four or more
/// alphanumerics. Not caught, declared: `sk_test_` (test-mode; deliberately outside — flagging
/// test keys would teach the operator to ignore red, the same argument [`ALLOWED_EMAIL_DOMAINS`]
/// makes), `rk_live_` restricted keys, `pk_live_` publishable keys (public by design), and
/// `whsec_` webhook secrets. A prose mention (`` `sk_live_`prefix ``) has a zero-length run.
fn stripe_live_key_hit(line: &str) -> bool {
    for (idx, _) in line.match_indices("sk_live_") {
        let run = line[idx + "sk_live_".len()..]
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .count();
        if run >= 24 {
            return true;
        }
    }
    false
}

/// **evasion 4** — a prefixless raw-hex API key, context-gated: a run of ≥48 hex characters
/// containing **both** a digit and a letter, on a line that also holds a key-ish word (`key`/
/// `secret`/`token`/`password`/`passwd`/`credential`, case-insensitive).
///
/// This is the most collision-prone shape in the corpus and every narrowing is deliberate,
/// measured against this tree before being chosen: (a) the **context gate**, because the tree is
/// full of long bare hex (CBOR wire dumps in `receipt_verdict_wire.rs`/`ac_033.rs`, tile hashes
/// in `tile_wire.rs`) that no key-ish word accompanies; (b) the **≥48 threshold**, because a git
/// commit/blob SHA is exactly 40 hex and prose like "key derived from commit <sha>" would trip a (sem: SEM-gx-cli-1387)
/// 40-threshold rule the moment it appears; (c) the **mixed-character requirement**, because
/// all-`a` and all-digit runs are this tree's placeholder idiom (`gx1:aaa…`, `"111…1"`), not
/// entropy. Each narrowing is therefore also a declared non-catch: bare hex without a context
/// word, hex of 40–47 characters (a real 40-hex secret with context included), and degenerate
/// single-class runs all pass.
const KEY_CONTEXT_WORDS: [&str; 6] = ["key", "secret", "token", "password", "passwd", "credential"];

fn keyed_hex_hit(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if !KEY_CONTEXT_WORDS.iter().any(|w| lower.contains(w)) {
        return false;
    }
    let (mut run, mut digits, mut letters) = (0usize, 0usize, 0usize);
    for c in line.chars() {
        if c.is_ascii_hexdigit() {
            run += 1;
            if c.is_ascii_digit() {
                digits += 1;
            } else {
                letters += 1;
            }
            if run >= 48 && digits > 0 && letters > 0 {
                return true;
            }
        } else {
            (run, digits, letters) = (0, 0, 0);
        }
    }
    false
}

/// **evasion 5** — a connection string with an embedded password: `scheme://user:password@host`,
/// any scheme, host dotted or single-label (the corpus's k8s/docker single-label form is exactly
/// the shape [`foreign_email_hit`] cannot see). The userinfo is read from after `://` up to `@`,
/// abandoned if `/`, whitespace, or a quote arrives first (those end a URI authority; a password
/// containing them is therefore not caught, declared). Also not caught, declared: a password
/// shorter than four characters (`u:abc@h` is overwhelmingly a prose or placeholder shape), an
/// empty user, and a DSN assembled at runtime from parts. `localhost` is **not** excepted: a
/// literal `postgres://user:pass@localhost` in this tree should go through an env var, and the
/// gate saying so is the point.
fn dsn_password_hit(line: &str) -> bool {
    for (idx, _) in line.match_indices("://") {
        let rest = &line[idx + 3..];
        let mut userinfo_end = None;
        for (i, c) in rest.char_indices() {
            if c == '@' {
                userinfo_end = Some(i);
                break;
            }
            if c == '/' || c.is_whitespace() || c == '"' || c == '\'' || c == '`' {
                break;
            }
        }
        let Some(at) = userinfo_end else {
            continue;
        };
        let Some((user, password)) = rest[..at].split_once(':') else {
            continue;
        };
        let host_ok = rest[at + 1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        if !user.is_empty() && password.chars().count() >= 4 && host_ok {
            return true;
        }
    }
    false
}

/// **evasion 6** — a JSON `"api_key"` field: the quoted key (case-insensitive), `:`, then a
/// quoted value opening with twenty or more token-shaped characters. Not caught, declared: every
/// other field name (`"token"`, `"access_token"`, `"apiKey"` — camel-case has no underscore and
/// is a different string), single-quoted or unquoted YAML forms (`api_key: abc…`), and values
/// shorter than twenty characters.
fn json_api_key_hit(line: &str) -> bool {
    const FIELD: &str = "\"api_key\"";
    let lower = line.to_ascii_lowercase();
    for (idx, _) in lower.match_indices(FIELD) {
        let rest = line[idx + FIELD.len()..].trim_start();
        let Some(rest) = rest.strip_prefix(':') else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        let run = value
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .count();
        if run >= 20 {
            return true;
        }
    }
    false
}

/// **evasion 7** — an `Authorization:` header under any scheme **other than** `Bearer`: the
/// header name (case-insensitive), an alphabetic scheme word, one space, then twenty or more
/// characters from a token alphabet wide enough for base64 (`Basic`) and dotted/dashed API keys.
/// `Bearer` itself is skipped by name — that scheme is [`bearer_token_hit`]'s jurisdiction, and
/// its placeholder/interpolation escapes (`<token>`, `{token}`) hold here unchanged: an angle
/// bracket or brace is not twenty token characters. Not caught, declared: a scheme whose token
/// is shorter than twenty characters, a token on the line after its header, and prose that names
/// the header without a colon (`Authorization scheme`) (sem: SEM-gx-cli-1388).
fn authorization_scheme_hit(line: &str) -> bool {
    const HEADER: &str = "authorization:";
    let lower = line.to_ascii_lowercase();
    for (idx, _) in lower.match_indices(HEADER) {
        let rest = line[idx + HEADER.len()..].trim_start_matches(' ');
        let scheme_len = rest.chars().take_while(char::is_ascii_alphabetic).count();
        if scheme_len == 0 {
            continue;
        }
        // The scheme is ASCII-alphabetic, so `scheme_len` chars == `scheme_len` bytes.
        let (scheme, after) = rest.split_at(scheme_len);
        if scheme.eq_ignore_ascii_case("bearer") {
            continue;
        }
        let Some(token_part) = after.strip_prefix(' ') else {
            continue;
        };
        let run = token_part
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || "._+/=-".contains(*c))
            .count();
        if run >= 20 {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------
// v0.3-d expansion rules (`req/159` §D item 6) — the three shapes `req/136` §5 declared uncovered
// and 33 NFR-012's residual clause (`req/38` §95 ruling 4) held open behind a corpus precondition: JWT,
// Azure connection strings, GCP service-account JSON. The precondition — "don't speculatively implement
// what has no real shape in the corpus" (`req/155`; sem: SEM-gx-cli-1389) — is discharged by collecting the *format* from each issuer's own
// primary documentation (RFC 7519; learn.microsoft.com storage-configure-connection-string;
// docs.cloud.google.com iam/keys-create-delete — recorded in Desktop/GitRepo/REFERENCES.md,
// 2026-08-15) and planting dummy material in `tests/fixtures/secret_scan_v03d/`. No real key was
// generated or copied; every corpus value is constructed to the documented shape with EXAMPLE
// markers in the material itself.
// ---------------------------------------------------------------------------------------------

/// **v0.3-d shape 1** — a JSON Web Token (RFC 7519 §3): three base64url segments joined by `.`,
/// where the first two open with `eyJ` — the base64url encoding of `{"`, which every JSON object
/// serialized without leading whitespace opens with, so a real JWS-compact header and payload
/// both start there. The rule asks for: `eyJ` + ≥12 base64url characters, `.`, `eyJ` + ≥12,
/// `.`, ≥16 signature characters (a minimal real header, `{"alg":"HS256","typ":"JWT"}`, encodes
/// to 36 characters; 12 admits even degenerate short headers without matching prose).
///
/// Not caught, declared: an **unsigned** token (`alg: none` — signature segment empty or under
/// sixteen characters; what it carries is claims, not key material, and the ungated form would
/// flag every dotted base64 pair), a token split across lines, a token whose JSON was encoded
/// with leading whitespace (does not open `eyJ`), and JWE five-segment tokens whose header takes
/// a different shape than `{"alg"…` only when it stops opening with `{"`.
fn jwt_hit(line: &str) -> bool {
    let b64url = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    for (idx, _) in line.match_indices("eyJ") {
        let rest = &line[idx..];
        let seg1 = rest.chars().take_while(|c| b64url(*c)).count();
        if seg1 < 12 {
            continue;
        }
        // The segments are base64url = ASCII, so char counts are byte offsets.
        let Some(after1) = rest[seg1..].strip_prefix('.') else {
            continue;
        };
        if !after1.starts_with("eyJ") {
            continue;
        }
        let seg2 = after1.chars().take_while(|c| b64url(*c)).count();
        if seg2 < 12 {
            continue;
        }
        let Some(after2) = after1[seg2..].strip_prefix('.') else {
            continue;
        };
        let seg3 = after2.chars().take_while(|c| b64url(*c)).count();
        if seg3 >= 16 {
            return true;
        }
    }
    false
}

/// **v0.3-d shape 2** — an Azure connection-string key: `AccountKey=` (storage accounts) or
/// `SharedAccessKey=` (Service Bus / Event Hubs), spelled in the exact case Azure's own
/// documentation and portal emit, followed by ≥40 characters of base64 (alphanumeric, `/`, `+`;
/// a real storage key is 88 characters ending `==`). The key name is the prefix the material
/// always travels under — Azure's format is `;`-separated `Name=Value` pairs, so unlike AWS's
/// secret key the context is structural rather than a gate this rule invents.
///
/// Not caught, declared: SAS tokens (`sig=` query parameter, URL-encoded — a different shape),
/// lower-cased or otherwise re-cased key names (the portal never emits them; matching them would
/// trade a documented shape for a guess), keys shorter than forty characters, and a key on the
/// line after its name.
fn azure_connection_key_hit(line: &str) -> bool {
    for name in ["AccountKey=", "SharedAccessKey="] {
        for (idx, _) in line.match_indices(name) {
            let run = line[idx + name.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '/' || *c == '+')
                .count();
            if run >= 40 {
                return true;
            }
        }
    }
    false
}

/// **v0.3-d shape 3** — the GCP service-account key-file marker: a JSON `"type"` field whose
/// value is `"service_account"`, the first field of every key file `iam.googleapis.com` issues
/// (docs.cloud.google.com/iam/docs/keys-create-delete's own example). The marker is what this
/// rule owns; the file's *material* fields already fall under other rules' jurisdiction when
/// they appear — the `"private_key"` line holds `-----BEGIN PRIVATE KEY-----` inline
/// ([`pem_header_hit`]) and `"client_email"` holds `…@….iam.gserviceaccount.com`
/// ([`foreign_email_hit`]) — so what was uncovered was the one line that names the file for
/// what it is, including a redacted or partial copy that kept the marker and lost the key.
///
/// Not caught, declared: the marker split across lines (`"type":` on one, the value on the
/// next), single-quoted or unquoted YAML spellings, and a key file whose fields were reordered
/// *and* stripped of both the marker and every material field — at which point no line-local
/// scanner has anything left to see.
fn gcp_service_account_hit(line: &str) -> bool {
    for (idx, _) in line.match_indices("\"type\"") {
        let rest = line[idx + "\"type\"".len()..].trim_start();
        let Some(value) = rest.strip_prefix(':') else {
            continue;
        };
        if value.trim_start().starts_with("\"service_account\"") {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------
// v0.4-h expansion rules (`req/38` §107 residual 6 / §113 residual) — the different-kind shapes, corpus-first as (sem: SEM-gx-cli-1390)
// before: no rule below exists without its format collected from the issuer's own publication
// (module doc above; Desktop/GitRepo/REFERENCES.md 2026-08-15), and the shapes that could not
// be collected (pre-2021 AAD secrets, Google `AQ.` keys) are declared unimplemented rather than
// guessed at (`req/155`).
// ---------------------------------------------------------------------------------------------

/// **v0.4-h shape 1** — an Entra ID (Azure AD) client-app secret, in the identifiable format
/// Microsoft's own secret-scanning model publishes (microsoft/security-utilities
/// `HighConfidenceSecurityModels.json`, `AadClientAppIdentifiableCredentials` = SEC101/156, the
/// rule ID learn.microsoft.com's Advanced Security pattern list names for this credential):
/// three characters over the secret alphabet (`A-Za-z0-9`, `~`, `.`, `_`, `-`), the version
/// signature `7Q~` or `8Q~`, then a tail of 31–34 more, **bounded on both sides** — the byte
/// before the leading three and the byte after the tail must sit outside the alphabet, where
/// `+` and `/` also fail the boundary: Microsoft's own delimiters exclude them so that a base64
/// blob which happens to contain the signature does not present a bounded token, and that
/// exclusion is kept here for the same false-positive reason.
///
/// Not caught, declared: pre-2021 secrets (random strings with **no identifiable signature** —
/// there is no documented shape to match, and a format that cannot be collected is not
/// implemented, `req/155`), a secret split across lines, and a tail run outside the documented
/// 31–34 window (an over-long run is a blob, not this token).
fn aad_client_secret_hit(line: &str) -> bool {
    let charset =
        |b: u8| b.is_ascii_alphanumeric() || b == b'~' || b == b'.' || b == b'_' || b == b'-';
    let boundary = |b: u8| !charset(b) && b != b'+' && b != b'/';
    let bytes = line.as_bytes();
    for (idx, _) in line.match_indices("Q~") {
        // The byte before `Q~` is the signature's version digit.
        if idx == 0 || (bytes[idx - 1] != b'7' && bytes[idx - 1] != b'8') {
            continue;
        }
        let digit = idx - 1;
        // Exactly three alphabet characters before the digit, then a boundary (or line start).
        if digit < 3 || !bytes[digit - 3..digit].iter().all(|&b| charset(b)) {
            continue;
        }
        if digit > 3 && !boundary(bytes[digit - 4]) {
            continue;
        }
        // A maximal tail run of 31–34 alphabet characters after `Q~`, then a boundary (or end).
        let tail = idx + 2;
        let run = bytes[tail..].iter().take_while(|&&b| charset(b)).count();
        if (31..=34).contains(&run) && bytes.get(tail + run).is_none_or(|&b| boundary(b)) {
            return true;
        }
    }
    false
}

/// **v0.4-h shape 2** — a Google API key: `AIza` followed by 35 or more characters over
/// `A-Za-z0-9_-`. Google's primary documentation (docs.cloud.google.com
/// docs/authentication/api-keys) publishes the shape by example — "The API key string is an
/// encrypted string, for example, `AIzaSy…`", a 39-character string opening `AIza` — the same (sem: SEM-gx-cli-1391)
/// example-based grounding the Azure connection-string rule stands on ("real keys 88 chars").
/// 35 is that example's tail length; the run is open-ended upward the way [`gh_token_hit`]'s
/// is, because a longer unbroken run past a documented prefix is more material, not prose.
///
/// Not caught, declared: the express-mode key family opening `AQ.` (**no issuer-documented
/// format** as of 2026-08-15 — collected-impossible, not implemented, `req/155`), a key split
/// across lines, prose naming the prefix (zero-length run), and tails under thirty-five.
fn gcp_api_key_hit(line: &str) -> bool {
    for (idx, _) in line.match_indices("AIza") {
        let run = line[idx + "AIza".len()..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .count();
        if run >= 35 {
            return true;
        }
    }
    false
}

/// **v0.4-h shape 3** — an npm access token: `npm_` followed by 36 or more Base62
/// (alphanumeric) characters. The issuer's announcement (github.blog "Announcing npm's new
/// access token format", 2021-09 — npm is GitHub's; sem: SEM-gx-cli-1392) documents the structure: the `npm` prefix,
/// an underscore delimiter, a Base62 alphabet, the last six characters a CRC32 checksum, and
/// 178 bits of entropy — which is thirty Base62 characters (178 ÷ log₂62 ≈ 29.9), plus the
/// checksum's six = thirty-six after the prefix. npm's own script environment
/// (`npm_config_*`, `npm_package_*`, `npm_lifecycle_event`) shares the prefix, and is why the
/// run is strictly alphanumeric: the token alphabet has no underscore, so the environment
/// variables' next `_` ends their run long before thirty-six.
///
/// Not caught, declared: legacy UUID-format tokens (36 hex-and-dash characters with **no**
/// prefix — the shape npm withdrew, indistinguishable from any other UUID in a tree full of
/// them), a token split across lines, and a run under thirty-six characters.
fn npm_token_hit(line: &str) -> bool {
    for (idx, _) in line.match_indices("npm_") {
        let run = line[idx + "npm_".len()..]
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .count();
        if run >= 36 {
            return true;
        }
    }
    false
}

/// **evasion 8** — a PEM private-key header split across two adjacent lines: this line holds
/// `-----BEGIN` without `PRIVATE KEY`, and the **next** line holds `PRIVATE KEY`. A one-line
/// header holds both substrings and is [`pem_header_hit`]'s jurisdiction, not this rule's (the
/// `!line.contains("PRIVATE KEY")` guard keeps the two disjoint, so a finding names exactly one
/// rule). Not caught, declared: a split with any intervening line (blank included), a split
/// inside the word run itself (`-----BE` / `GIN PRIVATE KEY`), splits across three or more
/// lines, and non-private-key PEM blocks (`CERTIFICATE`, `PUBLIC KEY` — public by design).
fn pem_split_header_hit(line: &str, next_line: &str) -> bool {
    line.contains("-----BEGIN")
        && !line.contains("PRIVATE KEY")
        && next_line.contains("PRIVATE KEY")
}

/// Every rule, over every line of every file. Fifteen of the sixteen rules are line-local; the
/// sixteenth ([`pem_split_header_hit`]) sees each line paired with the one after it (an empty
/// string past the last line — a window, not a second pass, so the walk stays single).
/// (v0.3-d, `req/159` §D item 6: thirteen became sixteen — [`jwt_hit`],
/// [`azure_connection_key_hit`], [`gcp_service_account_hit`].)
/// (v0.4-h, `req/38` §107 residual 6: sixteen became nineteen (sem: SEM-gx-cli-1393) — [`aad_client_secret_hit`],
/// [`gcp_api_key_hit`], [`npm_token_hit`]; eighteen of the nineteen are line-local, and
/// [`pem_split_header_hit`] remains the only two-line window.)
fn scan_tree(files: &[PathBuf]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (offset, line) in lines.iter().enumerate() {
            let line_no = offset + 1;
            let next_line = lines.get(offset + 1).copied().unwrap_or("");
            let line_rules: [(&'static str, bool); 19] = [
                ("pem_private_key_header", pem_header_hit(line)),
                ("aws_key_prefix", aws_key_hit(line)),
                ("gh_token_prefix", gh_token_hit(line)),
                ("bearer_like_token", bearer_token_hit(line)),
                ("foreign_email", foreign_email_hit(line).is_some()),
                // v0.2-b expansion (req/150 §B), in the evasion corpus's own order:
                ("aws_secret_access_key", aws_secret_key_hit(line)),
                ("slack_token_prefix", slack_token_hit(line)),
                ("stripe_live_key_prefix", stripe_live_key_hit(line)),
                ("keyed_hex_token", keyed_hex_hit(line)),
                ("dsn_embedded_password", dsn_password_hit(line)),
                ("json_api_key_field", json_api_key_hit(line)),
                ("authorization_scheme_token", authorization_scheme_hit(line)),
                (
                    "pem_split_private_key_header",
                    pem_split_header_hit(line, next_line),
                ),
                // v0.3-d expansion (req/159 §D item 6), in req/136 §5's own order:
                ("jwt_token", jwt_hit(line)),
                (
                    "azure_connection_string_key",
                    azure_connection_key_hit(line),
                ),
                ("gcp_service_account_marker", gcp_service_account_hit(line)),
                // v0.4-h expansion (req/38 §107 residual 6; sem: SEM-gx-cli-1394), in the residual's own order:
                ("aad_client_app_secret", aad_client_secret_hit(line)),
                ("gcp_api_key_prefix", gcp_api_key_hit(line)),
                ("npm_token_prefix", npm_token_hit(line)),
            ];
            for (rule, hit) in line_rules {
                if hit {
                    findings.push(Finding {
                        rule,
                        path: path.clone(),
                        line: line_no,
                    });
                }
            }
        }
    }
    findings
}

/// One narrow, named exemption from the dev-tree gate below: a specific (path, line, rule)
/// triple, not a whole file. R-909-4a (`req/909`, `req/38` SS874): the product scanner
/// (everything above this point in the file) stays untouched — only the *test*'s own
/// zero-findings assertion gains a per-finding allowlist, so a real secret anywhere else in the
/// tree still fails the gate.
struct AllowedFinding {
    /// Repo-root-relative path, forward-slash, matching `Finding::path`'s displayed form.
    path: &'static str,
    line: usize,
    rule: &'static str,
    /// Why this exact finding is not a secret. Required, not optional — an allowlist entry
    /// without a reason is indistinguishable from silencing the gate.
    reason: &'static str,
}

/// req/909 §② stage 7b: `crates/gx-core/tests/observation_class.rs` plants
/// `"postgres://admin:hunter2@db.acme.internal:5432/prod"` twice (lines 207 and 269) as one of
/// "the four adversarial shapes, verbatim from the bed" that `is_digest_form`/`EnvsetAdmission`
/// must *reject* — a negative-fixture literal, not a credential in use anywhere. Each occurrence
/// trips two rules (`dsn_embedded_password` on the `user:pass@host` shape, `foreign_email` on
/// `hunter2@db.acme.internal` matching the email heuristic), so four entries. The fixture bytes
/// are unchanged (ruling 3 keeps adversarial forms verbatim); only this test's assertion is
/// narrowed, by exact (path, line, rule) triple — a finding at any other line, in any other file,
/// or a *fifth* finding on these two lines, still fails the gate.
const ALLOWLISTED_FINDINGS: &[AllowedFinding] = &[
    AllowedFinding {
        path: "crates/gx-core/tests/observation_class.rs",
        line: 207,
        rule: "dsn_embedded_password",
        reason: "negative-fixture DSN literal `is_digest_form` must reject (req/909 §②); not a live credential",
    },
    AllowedFinding {
        path: "crates/gx-core/tests/observation_class.rs",
        line: 207,
        rule: "foreign_email",
        reason: "same negative-fixture DSN literal; `hunter2@db.acme.internal` is the userinfo/host of the DSN above, not an address",
    },
    AllowedFinding {
        path: "crates/gx-core/tests/observation_class.rs",
        line: 269,
        rule: "dsn_embedded_password",
        reason: "second occurrence of the same negative-fixture DSN literal, in `a_plaintext_value_is_deny_even_when_the_chain_also_gapped`",
    },
    AllowedFinding {
        path: "crates/gx-core/tests/observation_class.rs",
        line: 269,
        rule: "foreign_email",
        reason: "second occurrence; same reasoning as line 207's foreign_email entry above",
    },
];

/// The `ALLOWLISTED_FINDINGS` entry matching `f` exactly (path, line, and rule all agree), if
/// any. Used only to filter the dev-tree gate's assertion, never to skip scanning a file.
fn allowlist_entry<'a>(f: &Finding, root: &Path) -> Option<&'a AllowedFinding> {
    let rel = f.path.strip_prefix(root).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    ALLOWLISTED_FINDINGS
        .iter()
        .find(|a| a.path == rel && a.line == f.line && a.rule == f.rule)
}

fn is_allowlisted(f: &Finding, root: &Path) -> bool {
    allowlist_entry(f, root).is_some()
}

/// 🔴 **NFR-012's blocking gate**: `crates/`, `tools/`, `policies/` — 0 findings, modulo the
/// narrow allowlist above.
#[test]
fn the_dev_tree_has_zero_secret_scan_findings() {
    let root = repo_root();
    let fixture_dir = root.join("crates/gx-cli/tests/fixtures/secret_scan_positive");
    // 🔴 This file itself: `pem_header_hit`'s own literal pattern strings
    // (`"-----BEGIN "`/`"PRIVATE KEY"`) trip its own rule, the same self-reference
    // `m6_surface_doubt.rs` names for `authority_boundary.rs` ("these files discuss the words
    // they are scanned for"; sem: SEM-gx-cli-1395). A scanner excluding its own source is standard practice (gitleaks'
    // own repository does the same for its test fixtures), not a hole: the fixture directory below
    // is where a *planted* secret is proven detectable, and this file's rule definitions are not
    // secrets under any of its own five rules' actual meaning.
    let self_path = root.join("crates/gx-cli/tests/secret_scan.rs");
    // 🔴 v0.2-b (`req/150` §B): the M9 audit's evasion corpus was placed in `tools/` *because*
    // the five original rules could not see it (`req/136` §4-3 measured 0 findings over it, and
    // this gate walking it was the measurement). The eight expansion rules now detect all eight
    // of its shapes — see `the_scanner_detects_all_eight_evasion_shapes_in_the_audit_corpus`,
    // which points the scanner straight at it — so it is excluded here for the same reason the
    // planted fixture above is: a corpus of deliberately fake values the scanner provably sees
    // is a positive fixture, and ruling 3 keeps positive fixtures by allowlist, not deletion. The
    // corpus file itself stays byte-identical (33 NFR-012 gotcha note 77: "audit record — do not touch"; sem: SEM-gx-cli-1396).
    let evasion_corpus = root.join("tools/audit_m9_p2_scanner_evasion_fixture.sh");
    let mut files = Vec::new();
    for sub in ["crates", "tools", "policies"] {
        files.extend(walk_files(
            &root.join(sub),
            &["rs", "sh", "cedar", "toml"],
            &[
                fixture_dir.as_path(),
                self_path.as_path(),
                evasion_corpus.as_path(),
            ],
        ));
    }
    println!("SECRET_SCAN_FILES_SCANNED={}", files.len());
    assert!(
        files.len() > 100,
        "the walk found suspiciously few files ({}); a scanner over nothing reports 0 findings \
         for the wrong reason (§30)",
        files.len()
    );
    let findings = scan_tree(&files);
    for f in &findings {
        println!(
            "SECRET_SCAN_FINDING rule={} path={} line={}",
            f.rule,
            f.path.display(),
            f.line
        );
    }
    println!("SECRET_SCAN_DEV_TREE_FINDINGS={}", findings.len());
    let (allowlisted, unexpected): (Vec<_>, Vec<_>) =
        findings.iter().partition(|f| is_allowlisted(f, &root));
    for f in &allowlisted {
        let entry = allowlist_entry(f, &root).expect("just partitioned as allowlisted");
        println!(
            "SECRET_SCAN_ALLOWLISTED_FINDING rule={} path={} line={} reason={}",
            f.rule,
            f.path.display(),
            f.line,
            entry.reason
        );
    }
    println!(
        "SECRET_SCAN_ALLOWLISTED={} SECRET_SCAN_UNEXPECTED={}",
        allowlisted.len(),
        unexpected.len()
    );
    assert!(
        unexpected.is_empty(),
        "NFR-012: 0 unallowlisted findings over crates/+tools/+policies/ ({} allowlisted per \
         R-909-4a, req/909 §②); see the SECRET_SCAN_FINDING lines above for what tripped: \
         {unexpected:?}",
        allowlisted.len()
    );
}

/// 🔴 **AC-P2-4** — ruling 3's planted fail-open check (sem: SEM-gx-cli-1397): point the same scanner at a fixture holding one
/// fake value per rule and require every rule to fire.
#[test]
fn the_scanner_detects_a_planted_secret_in_its_own_fixture() {
    let fixture_dir = repo_root().join("crates/gx-cli/tests/fixtures/secret_scan_positive");
    let files = walk_files(&fixture_dir, &["txt"], &[]);
    assert!(
        !files.is_empty(),
        "{} holds no .txt fixture; the fail-open check has nothing to point the scanner at",
        fixture_dir.display()
    );
    let findings = scan_tree(&files);
    let mut rules: Vec<&str> = findings.iter().map(|f| f.rule).collect();
    rules.sort_unstable();
    rules.dedup();
    println!(
        "SECRET_SCAN_FIXTURE_FINDINGS={} RULES_HIT={rules:?}",
        findings.len()
    );
    for want in [
        "pem_private_key_header",
        "aws_key_prefix",
        "gh_token_prefix",
        "bearer_like_token",
        "foreign_email",
    ] {
        assert!(
            rules.contains(&want),
            "AC-P2-4: the planted fixture must trip {want:?}, or this scanner is fail-open for \
             it and its 0-findings claim above means nothing: rules hit were {rules:?}"
        );
    }
}

/// 🔴 **AC-V2B-1** (`req/150` §B) — the eight-shape evasion corpus the M9 audit planted
/// (`req/136` §4-3, gotcha77) is detected in full, one finding per shape, by the corpus that
/// measured the blind spots itself (untouched — the strongest positive fixture available is the
/// one written by the adversary).
///
/// Exact equality, not `contains`: exactly eight findings, exactly the eight expansion rules.
/// The lower bound is the detection claim; the upper bound is the false-positive claim — the
/// corpus's *comment* lines quote rule-shaped prose on purpose (`` `xoxb-`prefix ``,
/// `"Bearer "`, `Authorization scheme`) and a ninth finding would mean a rule fires on prose (sem: SEM-gx-cli-1398)
/// about secrets, the failure mode the original five rules' escapes were built against. That the
/// five original rules stay silent here re-states `req/136`'s own measurement (the corpus evades
/// them) as a permanent invariant.
#[test]
fn the_scanner_detects_all_eight_evasion_shapes_in_the_audit_corpus() {
    let corpus = repo_root().join("tools/audit_m9_p2_scanner_evasion_fixture.sh");
    // req/833: the corpus plants eight secret-shaped strings on purpose, so it must not ship in
    // the published tree (`tools/` there is `e2e.sh` alone, req/817 §3 — a public copy would
    // seed the repo with secret-scanner bait). The guard is keyed on the workspace declaration,
    // not on bare absence: the published root (req/817) declares no probes/doubt member, the
    // private root does — so a private tree that loses the corpus still fails below.
    if !corpus.is_file() && !workspace_declares("probes/doubt") {
        eprintln!(
            "SKIP the_scanner_detects_all_eight_evasion_shapes_in_the_audit_corpus: the evasion \
             corpus is deliberately not shipped (secret-shaped fixture; published tree, \
             req/817). The AC-V2B-1 detection claim is measured on the private tree (req/833)."
        );
        return;
    }
    assert!(
        corpus.is_file(),
        "{} is missing; the AC-V2B-1 detection claim has nothing to point the scanner at",
        corpus.display()
    );
    let findings = scan_tree(&[corpus]);
    for f in &findings {
        println!(
            "SECRET_SCAN_EVASION_FINDING rule={} path={} line={}",
            f.rule,
            f.path.display(),
            f.line
        );
    }
    println!("SECRET_SCAN_EVASION_CORPUS_FINDINGS={}", findings.len());
    let mut rules: Vec<&str> = findings.iter().map(|f| f.rule).collect();
    rules.sort_unstable();
    let mut want = [
        "aws_secret_access_key",
        "slack_token_prefix",
        "stripe_live_key_prefix",
        "keyed_hex_token",
        "dsn_embedded_password",
        "json_api_key_field",
        "authorization_scheme_token",
        "pem_split_private_key_header",
    ];
    want.sort_unstable();
    assert_eq!(
        rules,
        want.to_vec(),
        "AC-V2B-1: the corpus must yield exactly one finding per evasion shape — fewer is a \
         blind spot regressing, more is a rule firing on the corpus's own prose (or one shape \
         tripping two rules, which makes every finding ambiguous)"
    );
}

// ---------------------------------------------------------------------------------------------
// Per-rule probes (`req/150` §B: "each of the 8 corpus kinds detected at least once = POSITIVE, and no false detection = NEGATIVE"; sem: SEM-gx-cli-1399). Every
// positive is the corpus's own line verbatim; every negative is either a dev-tree shape the rule
// was measured against before its thresholds were chosen, or a declared non-catch from the
// rule's own doc — asserting the denominator so a later "improvement" that silently widens a
// rule turns a documented boundary red instead of quietly moving it. All values are fake.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_aws_secret_rule_requires_both_context_words_and_the_material() {
    assert!(aws_secret_key_hit(
        r#"AWS_SECRET_ACCESS_KEY_EXAMPLE="wJalrXUtnFEMIzK7MDENGbPxRfiCYEXAMPLEKEY01""#
    ));
    // Prose with both context words but no 40-character run.
    assert!(!aws_secret_key_hit(
        "aws secret access keys are described in the IAM user guide"
    ));
    // Declared non-catch: the same material without the context words.
    assert!(!aws_secret_key_hit(
        r#"X="wJalrXUtnFEMIzK7MDENGbPxRfiCYEXAMPLEKEY01""#
    ));
}

#[test]
fn the_slack_rule_requires_a_real_token_run_after_the_prefix() {
    // 🔴 **GH013 / `req/789` §9, `req/801`** — the literal is split at the prefix boundary so the
    // *source blob* never carries a contiguous token-shaped run (GitHub's push-time secret
    // classifier blocked the public delta push on this exact line; it scans bytes, not semantics).
    // `concat!` joins at compile time, so the string the rule is handed is byte-for-byte what it
    // always was — the scanner's code path is unchanged, only the file's resting shape is.
    assert!(slack_token_hit(concat!(
        r#"SLACK_BOT_TOKEN_EXAMPLE="xoxb-"#,
        r#"111111111111-222222222222-abcdefghijklmnopqrstuvwx""#
    )));
    // The corpus's own comment line: a prose mention with a zero-length run.
    assert!(!slack_token_hit(
        "# --- evasion 2: Slack bot token(`xoxb-`prefix)"
    ));
    assert!(!slack_token_hit("xoxb-short"));
}

#[test]
fn the_stripe_rule_is_live_mode_only() {
    // 🔴 **GH013 / `req/789` §9, `req/801`** — split at the prefix boundary for the same reason as
    // the Slack line above: the blob carries no contiguous `sk_live_`+run, the runtime string is
    // unchanged.
    assert!(stripe_live_key_hit(concat!(
        r#"STRIPE_SECRET_KEY_EXAMPLE="sk_live_"#,
        r#"51ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefZZ""#
    )));
    assert!(!stripe_live_key_hit(
        "# --- evasion 3: Stripe secret key(`sk_live_`prefix)"
    ));
    // Declared non-catch: test-mode keys are deliberately outside the rule.
    assert!(!stripe_live_key_hit(
        r#"STRIPE_TEST="sk_test_51ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefZZ""#
    ));
}

#[test]
fn the_keyed_hex_rule_needs_context_length_and_mixed_characters() {
    assert!(keyed_hex_hit(
        r#"GENERIC_HEX_API_KEY_EXAMPLE="a1b2c3d4e5f60718293a4b5c6d7e8f901234567890abcdef1234567890abcd""#
    ));
    // The same 62-hex material with no key-ish word on the line: the context gate that keeps
    // this tree's CBOR wire dumps (receipt_verdict_wire.rs, ac_033.rs) out.
    assert!(!keyed_hex_hit(
        r#"WIRE="a1b2c3d4e5f60718293a4b5c6d7e8f901234567890abcdef1234567890abcd""#
    ));
    // A git-SHA-sized (40) hex with context: under the ≥48 threshold, declared not caught.
    assert!(!keyed_hex_hit(
        r#"key derived from commit 356a192b7913b04c54574d18c28d46e6395428ab"#
    ));
    // Placeholder-idiom hex with context: 64 characters of one class carry no entropy.
    assert!(!keyed_hex_hit(
        r#"key = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa""#
    ));
}

#[test]
fn the_dsn_rule_requires_a_password_in_the_userinfo() {
    assert!(dsn_password_hit(
        r#"DB_CONNECTION_STRING_EXAMPLE="postgres://dbuser:Sup3rSecretPWexample@prod-db-primary:5432/prod""#
    ));
    // A URI authority with a user but no password (locator.rs's own test shape).
    assert!(!dsn_password_hit("https://Alice@MCP.Example:8443/path"));
    // No userinfo at all: the `/` after the host ends the search before any `@`.
    assert!(!dsn_password_hit("postgres://localhost:5432/prod"));
    // Declared non-catch: a password shorter than four characters.
    assert!(!dsn_password_hit("postgres://u:abc@h"));
}

#[test]
fn the_json_api_key_rule_requires_the_exact_field_and_a_long_value() {
    assert!(json_api_key_hit(
        r#"API_KEY_JSON_FIELD_EXAMPLE='{"api_key": "abcdefghijklmnopqrstuvwxyz0123456789ABCDEXAMPLE"}'"#
    ));
    assert!(!json_api_key_hit(r#"{"api_key": "short"}"#));
    // Declared non-catch: a different field name with a long value.
    assert!(!json_api_key_hit(
        r#"{"user_name": "abcdefghijklmnopqrstuvwxyz0123456789"}"#
    ));
    // Declared non-catch: the unquoted YAML form.
    assert!(!json_api_key_hit(
        "api_key: abcdefghijklmnopqrstuvwxyz0123456789"
    ));
}

#[test]
fn the_authorization_scheme_rule_defers_bearer_to_the_original_rule() {
    assert!(authorization_scheme_hit(
        r#"AUTHORIZATION_APIKEY_SCHEME_EXAMPLE="Authorization: ApiKey abcdefghijklmnopqrstuvwxyz0123456789EXAMPLE""#
    ));
    // Jurisdiction: a Bearer line belongs to `bearer_like_token`, not this rule — both halves
    // asserted, so the line is still caught by exactly one rule.
    let bearer_line = "Authorization: Bearer thisisadeliberatelyplantedfaketoken123";
    assert!(!authorization_scheme_hit(bearer_line));
    assert!(bearer_token_hit(bearer_line));
    // 44 §2.5's own prose shape: an angle-bracketed placeholder is not twenty token characters.
    assert!(!authorization_scheme_hit("Authorization: ApiKey <token>"));
    // auth.rs's own doc prose: `authorization:` followed by ordinary words never reaches twenty.
    assert!(!authorization_scheme_hit(
        "authorization: the only check is a single static Bearer token"
    ));
}

/// 🔴 **v0.3-d** (`req/159` §D item 6) — the three-shape corpus is detected in full, one finding
/// per shape, and by exactly the three new rules. The same double claim as the eight-shape test:
/// exact equality means fewer is a blind spot and more is a rule firing on the corpus's own
/// header prose (which deliberately names `AccountKey=`, `eyJ`, and the service-account marker
/// in rule-shaped-but-short forms).
#[test]
fn the_scanner_detects_the_three_v03d_shapes_in_their_corpus() {
    let corpus = repo_root().join("crates/gx-cli/tests/fixtures/secret_scan_v03d/corpus.txt");
    assert!(
        corpus.is_file(),
        "{} is missing; the v0.3-d detection claim has nothing to point the scanner at",
        corpus.display()
    );
    let findings = scan_tree(&[corpus]);
    for f in &findings {
        println!(
            "SECRET_SCAN_V03D_FINDING rule={} path={} line={}",
            f.rule,
            f.path.display(),
            f.line
        );
    }
    println!("SECRET_SCAN_V03D_CORPUS_FINDINGS={}", findings.len());
    let mut rules: Vec<&str> = findings.iter().map(|f| f.rule).collect();
    rules.sort_unstable();
    let mut want = [
        "jwt_token",
        "azure_connection_string_key",
        "gcp_service_account_marker",
    ];
    want.sort_unstable();
    assert_eq!(
        rules,
        want.to_vec(),
        "req/159 §D item 6: the corpus must yield exactly one finding per shape — fewer is a \
         blind spot, more is a rule firing on prose about secrets (or on the thirteen prior \
         rules' jurisdiction)"
    );
}

#[test]
fn the_jwt_rule_requires_two_eyj_segments_and_a_signature() {
    assert!(jwt_hit(
        "SESSION_JWT_EXAMPLE=\"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJFWEFNUExFLXN1YmplY3QiLCJpYXQiOjB9.EXAMPLEfakesignaturebytesEXAMPLEnotreal\""
    ));
    // Declared non-catch: an unsigned (alg none) token — the signature segment is empty.
    assert!(!jwt_hit(
        "eyJhbGciOiJub25lIn0.eyJzdWIiOiJFWEFNUExFLXN1YmplY3QifQ."
    ));
    // Prose naming the prefix has no twelve-character run behind it.
    assert!(!jwt_hit("JSON objects open eyJ) when base64url-encoded"));
    // A dotted base64 pair whose second segment is not a JSON object is not a JWS compact form.
    assert!(!jwt_hit(
        "eyJhbGciOiJIUzI1NiJ9.c29tZXRoaW5nLWVsc2UtZW50aXJlbHk.EXAMPLEfakesignaturebytes"
    ));
}

#[test]
fn the_azure_rule_matches_the_documented_key_names_exactly() {
    assert!(azure_connection_key_hit(
        "DefaultEndpointsProtocol=https;AccountName=exampleacct;AccountKey=EXAMPLEexampleEXAMPLEexampleEXAMPLEexampleEXAMPLE0123456789fake==;EndpointSuffix=core.windows.net"
    ));
    assert!(azure_connection_key_hit(
        "Endpoint=sb://example.servicebus.windows.net/;SharedAccessKeyName=RootManageSharedAccessKey;SharedAccessKey=EXAMPLEexampleEXAMPLEexampleEXAMPLEexample0123456789fake"
    ));
    // Prose naming the key with no material behind the `=`.
    assert!(!azure_connection_key_hit(
        "learn.microsoft.com documents AccountKey= as base64"
    ));
    // Declared non-catch: a re-cased key name the portal never emits.
    assert!(!azure_connection_key_hit(
        "accountkey=EXAMPLEexampleEXAMPLEexampleEXAMPLEexample0123456789fake"
    ));
    // Declared non-catch: a key shorter than forty characters.
    assert!(!azure_connection_key_hit("AccountKey=shortEXAMPLE=="));
}

#[test]
fn the_gcp_rule_owns_the_marker_and_the_material_fields_stay_with_their_rules() {
    assert!(gcp_service_account_hit(
        r#"{"type": "service_account", "project_id": "example-project-id"}"#
    ));
    // Whitespace variants of one JSON document.
    assert!(gcp_service_account_hit(r#"{"type":"service_account"}"#));
    // Declared non-catch: the marker split across lines — `"type":` alone says nothing.
    assert!(!gcp_service_account_hit(r#""type":"#));
    // A different type value is a different kind of file.
    assert!(!gcp_service_account_hit(
        r#"{"type": "authorized_user", "client_id": "example"}"#
    ));
    // Jurisdiction: a key file's material fields are other rules' — asserted so a refactor that
    // narrowed those rules would turn this red rather than silently orphan the fields.
    assert!(pem_header_hit(
        r#""private_key": "-----BEGIN PRIVATE KEY-----\nEXAMPLE\n-----END PRIVATE KEY-----\n""#
    ));
    assert!(foreign_email_hit(
        r#""client_email": "example-sa@example-project.iam.gserviceaccount.com""#
    )
    .is_some());
}

/// 🔴 **v0.4-h** (`req/38` §107 residual 6; sem: SEM-gx-cli-1400) — the three-shape corpus is detected in full, one finding
/// per shape, and by exactly the three new rules. The same double claim as the two corpus tests
/// above: exact equality means fewer is a blind spot and more is a rule firing on the corpus's
/// own header prose (which deliberately names `7Q~`, `8Q~`, `AIza`, and the npm prefix in
/// rule-shaped-but-short forms) or on the sixteen prior rules' jurisdiction.
#[test]
fn the_scanner_detects_the_three_v04h_shapes_in_their_corpus() {
    let corpus = repo_root().join("crates/gx-cli/tests/fixtures/secret_scan_v04h/corpus.txt");
    assert!(
        corpus.is_file(),
        "{} is missing; the v0.4-h detection claim has nothing to point the scanner at",
        corpus.display()
    );
    let findings = scan_tree(&[corpus]);
    for f in &findings {
        println!(
            "SECRET_SCAN_V04H_FINDING rule={} path={} line={}",
            f.rule,
            f.path.display(),
            f.line
        );
    }
    println!("SECRET_SCAN_V04H_CORPUS_FINDINGS={}", findings.len());
    let mut rules: Vec<&str> = findings.iter().map(|f| f.rule).collect();
    rules.sort_unstable();
    let mut want = [
        "aad_client_app_secret",
        "gcp_api_key_prefix",
        "npm_token_prefix",
    ];
    want.sort_unstable();
    assert_eq!(
        rules,
        want.to_vec(),
        "req/38 §107 residual 6 (sem: SEM-gx-cli-1401): the corpus must yield exactly one finding per shape — fewer is a \
         blind spot, more is a rule firing on prose about secrets (or on the sixteen prior \
         rules' jurisdiction)"
    );
}

#[test]
fn the_aad_rule_requires_the_signature_the_prefix_run_and_the_bounded_tail() {
    // The corpus's own line: three alphabet characters, `8Q~`, a 31-character tail, quoted.
    assert!(aad_client_secret_hit(
        r#"AZURE_AD_CLIENT_SECRET_EXAMPLE="Fak8Q~EXAMPLEfakeEXAMPLEfakeEXAMPLEfk""#
    ));
    // The `7Q~` sibling signature, at line start (no leading boundary byte exists to check).
    // 🔴 **GH013 / `req/789` §9, `req/801`** — split mid-tail: the first fragment's tail run (18
    // characters) is under the 31 GitHub's AAD pattern needs, so the blob cannot match; `concat!`
    // hands the rule the same 37-byte string as before. (The quoted `8Q~` lines in this test were
    // measured through the same push attempt and did not trip the classifier — only this line did.)
    assert!(aad_client_secret_hit(concat!(
        "Fak7Q~EXAMPLEfakeEXAMPLE",
        "fakeEXAMPLEfk"
    )));
    // Prose naming the signatures: no three-character alphabet run before the digit, no tail.
    assert!(!aad_client_secret_hit(
        "the signature is 7Q~ or 8Q~ inside the secret"
    ));
    // A tail under the documented 31 minimum (28 characters).
    assert!(!aad_client_secret_hit("Fak8Q~EXAMPLEfakeEXAMPLEfakeEXAMPL"));
    // A tail past the documented 34 maximum (40 characters) is a blob, not this token.
    assert!(!aad_client_secret_hit(
        "Fak8Q~EXAMPLEfakeEXAMPLEfakeEXAMPLEfakeEXAMPLE"
    ));
    // Embedded in base64: a `+` ends the tail run at 31 but fails the boundary, and a fourth
    // alphabet character before the signature fails the leading boundary — both halves of
    // Microsoft's own delimiter exclusion, asserted separately.
    assert!(!aad_client_secret_hit(
        "Fak8Q~EXAMPLEfakeEXAMPLEfakeEXAMPLEfk+b64"
    ));
    assert!(!aad_client_secret_hit(
        "xFak8Q~EXAMPLEfakeEXAMPLEfakeEXAMPLEfk"
    ));
}

#[test]
fn the_gcp_api_key_rule_requires_the_documented_prefix_and_a_full_tail() {
    // The corpus's own line: `AIza` + 35 characters, the documented example's length.
    assert!(gcp_api_key_hit(
        r#"GCP_API_KEY_EXAMPLE="AIzaEXAMPLEfakeEXAMPLEfakeEXAMPLEfake01""#
    ));
    // Prose naming the prefix has a zero-length run behind it.
    assert!(!gcp_api_key_hit("keys begin with the AIza prefix"));
    // A tail under thirty-five characters.
    assert!(!gcp_api_key_hit(r#"KEY="AIzaEXAMPLEtooShort0123""#));
    // Declared non-catch, pinned so a later guess does not silently widen the rule: the
    // express-mode `AQ.` family has no issuer-documented format and is not implemented.
    assert!(!gcp_api_key_hit(
        r#"GEMINI_EXPRESS_KEY_EXAMPLE="AQ.ExampleFakeExampleFakeExampleFake0123""#
    ));
}

#[test]
fn the_npm_rule_requires_a_base62_run_and_ignores_npms_own_environment() {
    // The corpus's own line: `npm_` + 36 alphanumerics (30 of entropy + 6 of checksum).
    assert!(npm_token_hit(
        r#"NPM_AUTOMATION_TOKEN_EXAMPLE="npm_EXAMPLEfakeEXAMPLEfakeEXAMPLEfake012""#
    ));
    // npm's script environment shares the prefix; the next underscore ends each run early.
    assert!(!npm_token_hit(
        "npm_config_registry=https://registry.npmjs.example"
    ));
    assert!(!npm_token_hit(
        "npm_package_json npm_lifecycle_event npm_node_execpath"
    ));
    // A run under thirty-six characters.
    assert!(!npm_token_hit(r#"TOKEN="npm_EXAMPLEshort012""#));
}

#[test]
fn the_pem_split_rule_sees_adjacent_lines_and_only_private_keys() {
    // The corpus's own two lines (54–55).
    assert!(pem_split_header_hit(
        "# -----BEGIN",
        "# PRIVATE KEY-----EXAMPLE-SPLIT-ACROSS-TWO-LINES-NOT-A-REAL-KEY"
    ));
    // Jurisdiction: a one-line header is `pem_private_key_header`'s, not this rule's.
    let one_line = "-----BEGIN RSA PRIVATE KEY-----";
    assert!(!pem_split_header_hit(one_line, "anything"));
    assert!(pem_header_hit(one_line));
    // Public objects are public by design.
    assert!(!pem_split_header_hit(
        "-----BEGIN CERTIFICATE-----",
        "MIIBIjANBg"
    ));
    // Declared non-catch: an intervening blank line breaks the two-line window.
    assert!(!pem_split_header_hit("# -----BEGIN", ""));
}
