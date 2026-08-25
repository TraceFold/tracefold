// 🔴 **`req/316` M-03 / `req/317` §1 item 4 (R24)** — a pack that denies **one** `mcp` locator a
// suite may actually write, so record-only can be measured on the surface where it means something.
//
// # Why a second fixture rather than `deny-writable.cedar`
//
// That pack permits `resource.substrate == "fs"` and forbids one fs locator. Cedar is default-deny,
// so under it *every* `mcp` transformation is denied — by the **absence of a permit**, not by a
// forbid anybody wrote. A record-only arm run under it would be measuring "no policy covers this
// substrate", and would go on passing if the forbid were deleted. Here the permit covers `mcp`, so
// the deny is the forbid below and a control call on a different resource is admitted by the same
// pack in the same run.
//
// M6H3-9 is filed as "a record-only E2E needs a policy pack over a writable path, and `gx verify`
// has no `--policy`". `gx wrap` **has** `--policy`, so on this surface the blocker does not hold;
// `docs/LIMITS.md` records that rather than leaving the note standing.
//
// `@id` on both statements because `PolicyEngine::parse` refuses a set without one (ASM-62-1), and
// because the deciding policy's id reaches `PolicyDecisionRecord` and from there a receipt's CID.
// The forbid matches a **name fragment** for `deny-writable.cedar`'s reason: the locator lives in a
// temporary directory that does not exist until the test runs, and `like` is Cedar's only string
// matcher over the resource attribute ASM-60-1 maps from `pre.locator()`.

@id("fixture-permit-mcp")
permit (
    principal,
    action,
    resource
)
when { resource.substrate == "mcp" };

@id("fixture-deny-mcp-note")
forbid (
    principal,
    action,
    resource
)
when {
    resource.substrate == "mcp"
    && resource.locator like "*gx-denied-note*"
};
