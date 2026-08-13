//! 🔴 **M6-10 採(a)+(b)** — one static Bearer token, a loopback default, and the absence in writing.
//!
//! 44 §2.5, verbatim:
//!
//! > **v0.1（ASM）**: 全エンドポイント（`/healthz`除く）に`Authorization: Bearer <token>`必須。
//! > トークン検証方式（静的トークン／ローカル鍵ストアとの対応付け等）は**未確定**。
//!
//! req/38 §47 adopted M6-10 as 「(a)+(b) の分離」: a static Bearer, no correspondence to `Actor`, the
//! **default bind fixed to loopback**, and M5H6-4(b) (a real authorization layer) registered in the
//! D-7 ledger against the firing condition 「loopback 以外に bind される時」.
//!
//! # 🔴 What this checks, and the much larger thing it does not
//!
//! One string comparison. It answers 「does the caller hold the server's token」 and **nothing** about
//! who they are: v0.1 has no authorization layer at all (M5H6-4 採(a)), `Engine::cancel` takes no
//! actor, 43 T-7's owner guard has no enforcement point, and the `actor` in a request body is the
//! caller's own claim about themselves. 還流台帳 §1.2 put it plainly — 「authorization 0 (cancel は
//! 誰でも押せる)…44 の API 面が生えた瞬間に必須」 — and the API面 is now here.
//!
//! [`ABSENCE_NOTICE`] is that sentence as a string the surface can print, because 卓-5's form is that
//! 「検査の不在を隠さない」 means the absence is **said** where the operator is, not only where the
//! reviewer is.
//!
//! # 🔴 Why the token is a start-up argument
//!
//! §47's brief for this hand fixes it: 「静的 Bearer(`.gx/config.toml` か env でなく**起動引数**)」.
//! Three roads and two are closed:
//!
//! * **the environment** — **M5H5-4(b)** refused environment variables as inputs to shipped
//!   behaviour, and `m6_surface_doubt::there_is_no_way_to_tell_this_binary_what_time_it_is` already
//!   scans gx-cli for the shape. A secret is not a clock, but the reason generalises: an input that
//!   arrives invisibly is an input nobody reviews.
//! * **`.gx/config.toml`** — req/56 §1 is 「秘密は project に置かない」 and §3 puts key material under
//!   `~/.gx/keys/` at 0600. A bearer token in a project file is a secret in the working tree, one
//!   `git add -A` away from a repository. req/56 §2's row for that file is 「project-local 設定
//!   (secrets 禁)」 and this is the sentence it forbids.
//! * **a start-up argument** — what is left. It is visible in `ps`, which is a real cost and is
//!   smaller than the other two; a token file whose *path* is the argument is the shape hand 6
//!   should reach for when `gx serve` grows its flags, and this type takes the token rather than the
//!   path so that the choice stays with the caller.
//!
//! # The default bind
//!
//! [`DEFAULT_BIND`] is loopback, and [`bind_refusal`] is the check M6-10(b) asks for. 44 §1.2 does
//! **not** write a default for `gx serve --bind`, and req/88 M6-10 named the consequence: 「未定義の
//! まま実装すると `0.0.0.0` が既定になりうる(authorization 無しで公開 network に出る)」. The runtime
//! is hand 6's; the policy is here, so that the hand which writes the flag is not also the hand that
//! decides what the flag defaults to.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::problem::ApiError;
use crate::state::AppState;

/// 🔴 44 §1.2 gives `gx serve --bind` no default; this is it (**M6-10 採(b)**).
pub const DEFAULT_BIND: &str = "127.0.0.1:8787";

/// 🔴 The sentence 44 §2.5's 「未確定」 turns into at the surface an operator actually reads.
///
/// req/38 §47's brief: 「『認可の検査は Bearer 1 本だけ』を `--help` と doc に明記」. A constant rather
/// than prose in a report, so that `gx serve --help` (hand 6, the hand with the flag) renders the
/// same words the crate documentation carries and `crates/gx-api/tests/auth.rs` asserts they exist.
pub const ABSENCE_NOTICE: &str = "\
authorization: the only check is a single static Bearer token (44 §2.5, v0.1). It answers whether \
the caller holds this server's token and nothing about who they are: there is no authorization \
layer in v0.1 (M5H6-4), `cancel` and `escalation` accept the actor the request declares, and 43 \
T-7's owner guard has no enforcement point. The default bind is therefore loopback \
(127.0.0.1:8787); binding anywhere else exposes an unauthorized surface and is refused without an \
explicit flag.";

/// 🔴 Whether an address may be bound without the explicit flag M6-10(b) requires.
///
/// `None` for a loopback address, `Some(reason)` otherwise. The refusal is returned as a **string a
/// caller prints** rather than raised, because the hand that owns the flag is hand 6 and a refusal
/// this hand raised would be a policy pretending to be a runtime.
///
/// The parse is deliberately literal: a host that is not obviously loopback is treated as not
/// loopback. Guessing in the permissive direction is how a default becomes `0.0.0.0`.
#[must_use]
pub fn bind_refusal(addr: &str) -> Option<String> {
    let host = addr.rsplit_once(':').map_or(addr, |(h, _)| h);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let loopback =
        host == "127.0.0.1" || host == "::1" || host == "localhost" || host.starts_with("127.");
    if loopback {
        return None;
    }
    Some(format!(
        "{addr} is not a loopback address, and v0.1 has no authorization layer beyond one static \
         Bearer token (44 §2.5 ASM-44-1, M5H6-4 採(a)). req/38 §47 registered the firing condition \
         for a real one as 「loopback 以外に bind される時」, so binding here needs an explicit flag \
         and an operator who has read {ABSENCE_NOTICE:?}"
    ))
}

/// The server's token, as a value that cannot be printed by accident.
///
/// No `Debug` derive and no `Display`: 45 §4's scan is about public文言 and this is about logs, but
/// the failure mode is the same one — a secret that is `{:?}`-able ends up in a trace the day
/// somebody adds one. The only operation is [`Bearer::matches`].
#[derive(Clone)]
pub struct Bearer {
    token: String,
}

impl std::fmt::Debug for Bearer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The length, because 「the token is empty」 is a deployment fault worth being able to see.
        write!(f, "Bearer(<{} bytes>)", self.token.len())
    }
}

impl Bearer {
    /// The token this server accepts, from the start-up argument (see the module header).
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }

    /// Whether a presented token is this one.
    ///
    /// 🔴 A **constant-time** comparison over equal-length inputs, and an early `false` on unequal
    /// lengths. `==` on `String` returns at the first differing byte, which leaks the shared prefix
    /// through timing; the length leak that remains is the one every fixed-token scheme has.
    /// v0.1's whole authentication is this function, so it is the one place the cost of being
    /// careful is obviously worth paying.
    #[must_use]
    pub fn matches(&self, presented: &str) -> bool {
        // A server with no token matches nothing, including the empty presented token. Without this
        // line an unset server would accept `Authorization: Bearer ` — authentication switched off
        // while reporting that it passed. `guard` refuses such a server outright; this makes the
        // refusal true of the primitive too, so removing the guard could not open the hole.
        if self.token.is_empty() {
            return false;
        }
        let (a, b) = (self.token.as_bytes(), presented.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut diff = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }

    /// Whether this server was started with no token at all.
    ///
    /// A separate question from [`Bearer::matches`], and the reason is 44 §2.5's 「必須」: an empty
    /// token would make every request with an empty header succeed, which is 「authentication is off」
    /// wearing the shape of 「authentication passed」. The router refuses to answer at all in that
    /// case (see [`guard`]).
    #[must_use]
    pub fn is_unset(&self) -> bool {
        self.token.is_empty()
    }
}

/// The header 44 §2.5 names.
const HEADER: &str = "authorization";

/// The scheme, with its trailing space.
const SCHEME: &str = "Bearer ";

/// 🔴 The middleware every endpoint but `/healthz` sits behind.
///
/// A **layer** rather than a line in each handler, and the difference is the whole of the guarantee:
/// thirteen handlers each remembering to call a checker is thirteen chances to forget, and the one
/// that forgets is not discoverable by reading the handler that did not. 44 §2.5's 「全エンドポイント
/// （`/healthz`除く）」 is a statement about the router's shape, so it is enforced on the router's
/// shape — `crates/gx-api/tests/auth.rs` walks every route and asserts each one refuses an
/// unauthenticated request.
///
/// # Errors
/// `401 UNAUTHORIZED` for a missing header, a header that is not `Bearer <token>`, or a token that
/// is not this server's. All three are one answer on purpose: naming which of the three failed tells
/// a prober how to get closer, and gx-witness's `SignatureInvalid` makes the identical argument
/// about four failures.
pub async fn guard(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let bearer = state.bearer();
    if bearer.is_unset() {
        // 🔴 Not 401: a server with no token has not refused this caller, it has been started
        // wrongly. Answering 401 would tell an operator 「your token is wrong」 about a server that
        // has none, which is M4H4-2's 「未実装と失敗を同じ顔にするな」 at the deployment layer.
        return Err(ApiError::internal(
            "this server was started without a Bearer token and 44 §2.5 makes one required on \
             every endpoint but /healthz. A server that accepted the empty token would be running \
             with authentication off while reporting that it passed",
        ));
    }
    let presented = request
        .headers()
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix(SCHEME));
    match presented {
        Some(token) if bearer.matches(token) => Ok(next.run(request).await),
        _ => Err(ApiError::unauthorized()),
    }
}
