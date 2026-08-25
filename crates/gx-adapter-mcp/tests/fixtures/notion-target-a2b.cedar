// The makenotion/notion-mcp-server **A2b example pack** (v0.4-b, req/170 §3-B, req/183's E2E) --
// the same shape, reasons and placement as the github packs beside it (`github-target.cedar`'s
// header carries the standing argument: why a target example lives in tests/fixtures/ rather than
// policies/, and why "only a catalogue tool is permitted" is not cedar-expressible -- the tool's name is (sem: SEM-gx-adapter-mcp-018)
// opaque to a policy, 42 §3.4).
//
// One narrow permit over Cedar's own default-deny, scoped to the **one throwaway parent page**
// this lane created in the operator's workspace (a row named `glovrex-throwaway-parent-2026-08-15`
// in the only database the "Glovrex" integration is connected to; trashed by the caller after the
// lane, the way the github throwaway repositories were archived). The subject locator of every
// call the E2E makes is this parent -- an *existing* position, gotcha93's rule -- and the pages
// the forward call creates are its children, so scoping the permit to the parent's id is the
// exact analogue of the github packs' repository prefix.
//
// 🔴 The resource URI here (`notion://pages/<uuid>`) is **synthetic**: notion-mcp-server declares
// no `resources` capability at all (`initialize` answers `{"tools":{}}`; `resources/read` is
// -32601 Method not found -- measured, req/183 §R0), so nothing on the server side ever names this
// URI. gx's claim through it is "this change is about this object" (`ToolCall::resource`'s own (sem: SEM-gx-adapter-mcp-019)
// doc), the same claim the github issue pair made through the repository README. What the
// absence of a read surface does to `gx wrap` (snapshot/precondition cannot read the subject at
// all) is the subject of req/183's DR-V4B-1, not of this pack.
//
// Notion-Version pin: the server this pack was measured against (commit 1d38420, package 2.5.1)
// sends `Notion-Version: 2025-09-03` per operation from its own OpenAPI spec (`http-client.ts`,
// header parameters are server-managed and not tool inputs). The catalogue JSON beside this file
// has no metadata slot to carry that pin (`Catalogue::from_json` is a bare tool->spec map), so the
// pin lives here and in `tools/a2b_notion_undo_e2e.sh`'s preflight, which asserts the checked-out
// server's spec default before anything is sent.
//
// As in the github packs, this permit pins no server *endpoint* (the leading `*` before `#`):
// the footprint is commutation/deployment wiring's responsibility (req/38 §94, ruling 3, gotcha78). (sem: SEM-gx-adapter-mcp-020)

@id("notion-target-a2b-permit-throwaway-parent")
permit (
    principal,
    action,
    resource
)
when {
    resource.substrate == "mcp"
    && resource.locator like "*#notion://pages/3bd8cc9a-40ed-81b8-b572-f339da4f57ac*"
};
