// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-V4B-3**, the `gx wrap` half (`req/38` §123 ruling 3, `req/189`): which member the proxy
//! takes off the wire form of a `tools/call`, decided by one rule a test can hold without a server.
//!
//! The wire half (`gx_mcp_wire::WireTransport::stripping`) is measured against the strict-body
//! probe server in `crates/gx-mcp-wire/tests/dr_v4b_3_strip.rs`; this file pins the **policy**
//! `gx wrap` applies on top of it: a `gx_*`-named `--resource-arg` is stripped by default, a real
//! tool argument never is, and `--forward-resource-arg` turns the default off.

use gx_cli::wrap::stripped_member;

#[test]
fn a_gx_named_resource_arg_is_stripped_by_default_and_forwarded_on_request() {
    assert_eq!(
        stripped_member("gx_resource_uri", false),
        Some("gx_resource_uri".to_string()),
        "the convention every tools/a*_e2e.sh uses is gx's member, not the tool's: stripped"
    );
    assert_eq!(
        stripped_member("gx_resource_uri", true),
        None,
        "--forward-resource-arg: sent as the agent wrote it (a server that declares it, or an \
         operator who wants the R7 failure visible)"
    );
    assert_eq!(
        stripped_member("gx_target", false),
        Some("gx_target".to_string())
    );
}

#[test]
fn a_real_tool_argument_named_as_the_resource_arg_is_never_stripped() {
    for real in [
        "uri",
        "path",
        "page_id",
        "GX_RESOURCE_URI",
        "resource_uri",
        "",
    ] {
        assert_eq!(
            stripped_member(real, false),
            None,
            "{real:?} is (or may be) the tool's own argument: taking it off would break the call \
             the other way -- 44 §1.2's default `--resource-arg uri` is exactly this case"
        );
        assert_eq!(stripped_member(real, true), None);
    }
}
