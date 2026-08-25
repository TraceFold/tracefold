// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-21** (`req/38` §218 ruling 2, §417/§421, `req/593` §11-§16, `req/679`) — the
//! compare-and-set half binds a **content-addressed** read's answer to the object it is keyed by,
//! by **digest re-verification**: the bytes the declared read answers must hash to the CID the
//! locator is keyed by, or the read is refused.
//!
//! # Why this is digest re-verification and not the escrow road's `ObjectIdentity`
//!
//! `dr46_21_identity_binding_gap.rs` arm 2 proved the escrow predicate cannot be reused here: a CAS
//! read answers the object's **raw bytes**, not a `{id,…}` document, so `ObjectIdentity` — which
//! reads `/id` out of a JSON answer — cannot even parse it. The binding a CAS read already carries
//! is the one content-addressing gives for free: the object's identity **is** its digest. So the
//! check is `content_digest(answer) == the CID the locator is keyed by`, and nothing about the
//! escrow road's answer shape is assumed.
//!
//! # The opt-in boundary, held by its own arms (`req/38` §421)
//!
//! The check is opt-in **by construction**: it fires exactly when the value keying the read is
//! itself a `gx1:` CID (the deployment content-addressed the object). Ruling (1): it is opt-in and
//! not mandatory, so a name-keyed `$cas_read` (`notion://page/abc-123`, whose key is not a CID) is
//! returned unchanged and the 163 name-keyed declarations do not move —
//! [`a_name_keyed_dishonest_read_is_returned_unchanged`]. Ruling (2): that name-keyed case is a
//! documented residual (`docs/LIMITS.md`), not a closed gap, and the arm above is what keeps this
//! file honest about which half DR-46-21 closes.

use std::sync::{Arc, Mutex};

use gx_adapter_mcp::adapter::content_digest;
use gx_adapter_mcp::{
    Admitted, CasArgSource, CasRead, CasTemplate, Catalogue, McpAdapter, ToolCall, ToolTransport,
};
use gx_substrate::{Error, Result, SubstrateAdapter};

const SERVER: &str = "https://mcp.example/dr4621";
const READ_TOOL: &str = "cas-read-object";

/// The bytes the locator is about, and a neighbour's — the `req/269` H-01 shape one road over.
const PAGE_BODY: &[u8] = b"the object this content-address names\n";
const NEIGHBOUR_BODY: &[u8] = b"a stranger's bytes the server answered instead\n";

/// The distinctive clause the refusal carries, pinned as a literal so this bed compiles at the base
/// commit (where the constant does not yet exist) and still names the fault it is looking for.
const REFUSAL_CLAUSE: &str = "content-addressed locator names";

fn locator(resource: &str) -> String {
    format!("{SERVER}#{resource}")
}

/// A tools-only MCP server whose read-by-tool face answers a fixed body regardless of the argument
/// it is handed. `answer == PAGE_BODY` is the honest server; `answer == NEIGHBOUR_BODY` is the
/// dishonest one DR-46-21 is about — handed the argument built from the locator, it answers about a
/// different object anyway, and the answer is opaque bytes so nothing in it says which object.
#[derive(Debug)]
struct CasServer {
    answer: Vec<u8>,
    reads: Mutex<usize>,
}

impl CasServer {
    fn honest() -> Self {
        Self {
            answer: PAGE_BODY.to_vec(),
            reads: Mutex::new(0),
        }
    }

    fn dishonest() -> Self {
        Self {
            answer: NEIGHBOUR_BODY.to_vec(),
            reads: Mutex::new(0),
        }
    }
}

impl ToolTransport for CasServer {
    fn read(&self, _server: &str, resource: &str) -> Result<Vec<u8>> {
        // Tools-only: no resource face, so the only road to a snapshot is the declared CAS read.
        Err(Error::Unreadable {
            locator: resource.to_string(),
            detail: "this server declares no `resources` capability (-32601)".to_string(),
        })
    }

    fn read_prior_by_tool(&self, _server: &str, _tool: &str, _arguments: &[u8]) -> Result<Vec<u8>> {
        *self.reads.lock().expect("the read count") += 1;
        Ok(self.answer.clone())
    }

    fn call(&self, _call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
        Ok(b"{\"ok\":true}".to_vec())
    }
}

/// A `$cas_read` keyed by the object's **digest**: the prefix is `cas://` and the suffix is the
/// `gx1:` spelling of `content_digest(body)`, so the value keying the read is a CID and the check
/// opts in. `id` carries that suffix to the tool, exactly as a content-addressed store's read is
/// called.
fn content_addressed(body: &[u8]) -> (Catalogue, String) {
    let key = content_digest(body).to_text();
    let resource = format!("cas://{key}");
    let catalogue = Catalogue::new().with_cas_read(
        "cas://",
        CasRead::new(
            READ_TOOL,
            CasTemplate::new().with("id", CasArgSource::ResourceSuffix),
        ),
    );
    (catalogue, resource)
}

/// 🔴 **The red-first arm.** A content-addressed read that answers about the wrong object is
/// **refused**, fail-closed. At the base commit the CAS road does not re-verify, so the neighbour's
/// bytes become this locator's digest and `snapshot` returns `Ok` — this arm panics there. After the
/// binding lands, `content_digest(neighbour) != key_cid` and the read is refused with the clause
/// pinned above.
#[test]
fn the_neighbours_bytes_are_refused_when_the_locator_is_content_addressed() {
    let (catalogue, resource) = content_addressed(PAGE_BODY);
    let mcp = McpAdapter::new(Arc::new(CasServer::dishonest())).with_catalogue(catalogue);

    match mcp.snapshot(&locator(&resource)) {
        Err(e) => {
            let rendered = e.to_string();
            println!("DR4621_REVERIFY refused={rendered}");
            assert!(
                rendered.contains(REFUSAL_CLAUSE),
                "🔴 the read was refused but not for the digest reason: {rendered:?}"
            );
        }
        Ok(snap) => panic!(
            "🔴 DR-46-21: the neighbour's bytes became this object's digest ({:?}) and nothing \
             refused it — the content-addressed read was not re-verified",
            snap.digest()
        ),
    }
}

/// The positive control (`req/593` AC-4′): an honest content-addressed read still snapshots, and the
/// digest it reports is the one the locator names. A binding that refused this would be closing the
/// road it is meant to keep open.
#[test]
fn an_honest_content_addressed_read_still_snapshots() {
    let (catalogue, resource) = content_addressed(PAGE_BODY);
    let mcp = McpAdapter::new(Arc::new(CasServer::honest())).with_catalogue(catalogue);

    let snap = mcp
        .snapshot(&locator(&resource))
        .expect("an honest content-addressed read snapshots on both roads");
    assert_eq!(
        *snap.digest(),
        content_digest(PAGE_BODY),
        "the reported digest is the object the locator content-addresses"
    );
}

/// The digest binding covers the **funnel**, not one site: a content-addressed dishonest read is
/// refused at `precondition` (verdict time) the same way it is at `snapshot`. The precondition is
/// taken over a snapshot an honest read produced, so the only thing that changed between the two
/// reads is the server's honesty.
#[test]
fn precondition_over_a_content_addressed_read_is_refused_too() {
    let (catalogue, resource) = content_addressed(PAGE_BODY);
    let honest = McpAdapter::new(Arc::new(CasServer::honest())).with_catalogue(catalogue.clone());
    let snap = honest
        .snapshot(&locator(&resource))
        .expect("the honest snapshot the precondition is taken over");

    let dishonest = McpAdapter::new(Arc::new(CasServer::dishonest())).with_catalogue(catalogue);
    match dishonest.precondition(&snap) {
        Err(e) => {
            let rendered = e.to_string();
            println!("DR4621_REVERIFY precondition_refused={rendered}");
            assert!(
                rendered.contains(REFUSAL_CLAUSE),
                "🔴 precondition was refused but not for the digest reason: {rendered:?}"
            );
        }
        Ok(fp) => panic!(
            "🔴 DR-46-21: precondition digested a stranger's bytes ({fp:?}) — the funnel's binding \
             did not reach the verdict-time read"
        ),
    }
}

/// 🔴 **The residual, held honest (`req/38` §421 ruling (2)).** A **name-keyed** `$cas_read`
/// (`notion://page/abc-123`, whose key is not a `gx1:` CID) does **not** opt in, so a dishonest
/// server's neighbour bytes still become this object's digest and `snapshot` returns `Ok`. This is
/// the documented `docs/LIMITS.md` limitation, not a closed gap: digest re-verification binds only
/// what is genuinely content-addressed, and this arm is what keeps the file from claiming otherwise.
/// It also holds the opt-in boundary from the other side — `req/38` §421 ruling (1): the check does
/// not fire on a name-keyed declaration, so the 163 of them are byte-for-byte where they were.
#[test]
fn a_name_keyed_dishonest_read_is_returned_unchanged() {
    let catalogue = Catalogue::new().with_cas_read(
        "notion://page/",
        CasRead::new(
            READ_TOOL,
            CasTemplate::new().with("page_id", CasArgSource::ResourceSuffix),
        ),
    );
    let mcp = McpAdapter::new(Arc::new(CasServer::dishonest())).with_catalogue(catalogue);

    let snap = mcp
        .snapshot(&locator("notion://page/abc-123"))
        .expect("a name-keyed read does not opt in to digest re-verification");
    assert_eq!(
        *snap.digest(),
        content_digest(NEIGHBOUR_BODY),
        "the residual DR-46-21 discloses: a name-keyed dishonest read is not bound by the digest"
    );
}
