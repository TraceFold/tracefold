// The github/github-mcp-server **A-2′ example pack** (req/38 §98, ruling 1 + §99, ruling 1, two-phase (sem: SEM-gx-adapter-mcp-007)
// escrow; req/162's E2E) -- the same shape, reasons and placement as `github-target-a2.cedar`
// beside it (whose header carries the standing argument: why a target example lives in
// tests/fixtures/ rather than policies/, and why "only a catalogue tool is permitted" is not (sem: SEM-gx-adapter-mcp-008)
// cedar-expressible in v0.1 -- the tool's name is opaque to a policy, 42 §3.4).
//
// One narrow permit over Cedar's own default-deny, scoped to the **A-2′** throwaway repository
// (`mahirhir/glovrex-a4-throwaway`, created for req/162's real-server issue-pair undo E2E and
// archived after it). A separate file rather than a second statement in the A2 pack for the same
// per-lane reason that pack gives: each lane's E2E is scoped to exactly the one repository it may
// write, and a pack carrying both repos would permit a retired one again.
//
// 🔴 This permit pins no server *endpoint* (the leading `*` before `#` admits any endpoint naming
// this repository resource) -- deliberate for a resource-scoped example: the footprint (which
// server a call may reach) is a different mechanism's responsibility (commutation/deployment
// wiring), not this pack's silence (req/38 §94, ruling 3, req/154 F4, gotcha78's family). (sem: SEM-gx-adapter-mcp-009)

@id("github-target-a4-permit-throwaway-repo")
permit (
    principal,
    action,
    resource
)
when {
    resource.substrate == "mcp"
    && resource.locator like "*#repo://mahirhir/glovrex-a4-throwaway/*"
};
