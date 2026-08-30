# attach interface fixtures — where each byte came from

`req/948c` AC-C6 and `req/969` AC-E10: every fixture declares whether it was **collected** from the
world or **constructed** from a specification. A green run over constructed bytes proves the reader
agrees with our reading of a document; only collected bytes make it a claim about the world.

| fixture | origin | sha256 (LF, as committed) | bytes | collected on |
|---|---|---|---|---|
| `github_attestations_response.json` | **collected** (derived: see redaction below) | `a21b6253e30d6a87645972ad6ae3cac2f124f5b48fce4409da0311eb20de314f` | 417 | 2026-08-30 |
| `github_bundle_resolved.json` | **collected**, then decompressed (see caveat) | `ef0f0b3cb8ffd14dee213f11a2b9f8ceef3bfb7c0728e5f85bce8df3c3849e9c` | 6305 | 2026-08-30 |

🔴 **The digests above are of the LF form, which is the form committed.** This repository sets
`core.autocrlf=true`, so a fixture written on Windows with a CRLF line ending is silently rewritten
on its way into the index — and a digest recorded before that happens is a digest of bytes nobody
will ever check out again. The response file had exactly one CRLF pair (its trailing newline) and
was normalised to LF before its digest was taken here; the bundle file had none and is unchanged.
**Record the digest of the bytes the reader will get, not of the bytes you happened to write.**

Everything built inline inside `tests/attach_interface.rs` is **constructed** and says so in the
module header there. The two files here are the only collected bytes this crate holds.

## What was collected

An anonymous `GET https://api.github.com/repos/cli/cli/attestations/sha256:3b8ac6b3...9208de`
(HTTP 200, no token). The digest itself came from the published `gh_2.98.0_checksums.txt` of the
`cli/cli` v2.98.0 release, so the subject asked about is a real released artifact
(`gh_2.98.0_linux_amd64.tar.gz`). Full method, headers and the unredacted originals:
`req/948b_artifacts/r1_real_response_2026-08-30.md` and the artifacts beside it.

## Redaction (why this file is *derived* and not byte-identical)

Each `bundle_url` in the response is an Azure Blob URL carrying a time-limited storage SAS
signature. The query string is replaced by the literal `REDACTED-STORAGE-SAS-QUERY`; nothing else is
altered. The signature had already expired when it was collected and grants read-only access to a
public artifact, so it is not a live secret — it is removed because a credential-shaped string does
not belong in a crate that gets published, not because it is dangerous.

The unredacted original is `req/948b_artifacts/gh_2.98.0_linux_amd64_tar_gz_attestations_response.json`
(sha256 `e3266246f3b2118192e3b141b5ff402f17c2844b6486472199298e4f03fd46df`), which is where to look
to check that the redaction touched only the query string.

**The structure under test is untouched by this**: `attestations[]` with two entries, each carrying
`repository_id`, a `bundle_url` string, an `initiator`, and — the point of the fixture —
`"bundle": null`.

## 🔴 Caveat on `github_bundle_resolved.json`

The bytes behind `bundle_url` arrive as `Content-Type: application/x-snappy`. This workspace has no
Snappy decoder and will not grow a dependency for one, so the file here is the output of a
**hand-written** decoder (`req/948b_artifacts/r1_real_response_2026-08-30.md` §1-1), checked three
ways: the length matches the varint the stream declares (6305), the output parses as JSON, and the
literal segments readable in the compressed bytes appear unchanged in the output.

**It has never been compared against a reference Snappy implementation.** A re-implemented decoder
measures our understanding of a format rather than the format
(`SKILL.md` — *run their gate, do not reimplement it*), so this fixture is collected-and-derived,
and the derivation is the weakest link in it. What it is used for — that a resolved bundle projects
onto four questions with the subject bound — does not depend on the decompression being bit-exact,
because a wrong decompression would not parse as JSON and would not contain the requested digest.
