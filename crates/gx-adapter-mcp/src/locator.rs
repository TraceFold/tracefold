// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Lexical locator normalisation for MCP: the `≈` of 42 §2.3, as a function (**E-M4-12**).
//!
//! The four clauses are in the crate root, where 42 §2.3 requires them and where an implementor reads
//! them; this module is their implementation and nothing else. It performs **no I/O**, which is what
//! keeps a scope computation from depending on a read taken at a different moment than the CAS check
//! it feeds.
//!
//! # Why the clauses are RFC 3986's and not this hand's
//!
//! M3-10 fixes a policy pack's effective range at "the locator level". Two spellings that this function (sem: SEM-gx-adapter-mcp-053)
//! leaves apart are therefore two policy subjects, and a rule written on one is not in force on the
//! other -- which, for the adapter whose acceptance criterion is "no bypass route technically exists" (sem: SEM-gx-adapter-mcp-054)
//! (AC-051), is a road around a policy rather than a cosmetic difference. So the definition of `≈` is
//! taken from the document that defines when two URIs are the same URI, and its section numbers are
//! quoted at each clause below.
//!
//! The stopping point is equally deliberate. RFC 3986 §6.2.3 (scheme-based) would make `https://x` and
//! `https://x/` one position and would elide `:443`; §6.2.4 (protocol-based) would call two endpoints
//! equal when they answer alike. Both require knowing what a scheme *means*, and an adapter that
//! assumed `https` semantics for a `stdio://` endpoint would merge two positions a deployment
//! distinguishes -- a normaliser that merges is worse than one that splits, because the split is
//! visible to a policy author and the merge is not.

use gx_substrate::{Error, Result};

/// The separator between the server endpoint and the resource.
///
/// `#` for the git adapter's reason one substrate over: it is the character neither part spends on
/// its own structure often enough to make splitting ambiguous. 🔴 It is **not** free of ambiguity here
/// -- RFC 3986 spends `#` on the fragment -- and [`parse`] refuses a resource that carries one rather
/// than guessing which `#` was the separator.
pub const SERVER_SEPARATOR: char = '#';

/// What a server endpoint has to carry to be a name rather than a nickname (crate root).
pub const SCHEME_SEPARATOR: &str = "://";

/// A locator, split into the two things it names.
///
/// Constructed only by [`parse`], so a value of this type is always **normalised**: there is no
/// spelling of a `Position` that [`normalize`] would change. That is what lets `apply`, `invert` and
/// `commutation` compare positions by comparing strings, and lets the crate root's `≈` be a fact about
/// the type rather than a discipline about call sites.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    server: String,
    resource: String,
}

impl Position {
    /// The MCP server endpoint, normalised.
    ///
    /// 🔴 This is the **footprint** of a change and not its scope: the crate root argues why a tool's
    /// effects are the server's semantics and not the resource's, and [`crate::commutation`] compares
    /// this value.
    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    /// The resource URI, normalised. The object, and the CAS scope (crate root's table).
    #[must_use]
    pub fn resource(&self) -> &str {
        &self.resource
    }

    /// The whole locator, in the one spelling this adapter writes.
    #[must_use]
    pub fn locator(&self) -> String {
        format!("{}{SERVER_SEPARATOR}{}", self.server, self.resource)
    }
}

/// The representative spelling of a locator (crate root, clauses 1-4).
///
/// Total: every string has a normal form, including one this adapter would refuse to act on. That is
/// L7's shape -- "normalising twice changes nothing" over *every* string -- and the refusal of a (sem: SEM-gx-adapter-mcp-055)
/// locator that is not a position happens where the locator is **used** ([`parse`]), not where it is
/// spelled. A normaliser that refused would make L7 a property with exceptions.
#[must_use]
pub fn normalize(locator: &str) -> String {
    match locator.split_once(SERVER_SEPARATOR) {
        Some((server, resource)) => format!(
            "{}{SERVER_SEPARATOR}{}",
            normalize_uri(server),
            normalize_uri(resource)
        ),
        // No separator at all: normalise the whole thing as one URI, so that the normal form of a
        // spelling this adapter cannot act on is still stable under a second normalisation.
        None => normalize_uri(locator),
    }
}

/// Read a locator as the two things it names, or say why it is not a position.
///
/// # Errors
/// [`Error::NotAPosition`] when the spelling names fewer than two parts, when the server endpoint
/// carries no scheme (crate root, ASM-69-3's argument), when either part is empty, or when the
/// resource carries a `#` of its own. All four are "the argument is not a position" and not "the apply failed" (sem: SEM-gx-adapter-mcp-056)
/// (**M4H5-5, adopted (b)**): 43 T-11 turns `ApplyFailed` into `AbortReason::ApplyFailed`, which would record (sem: SEM-gx-adapter-mcp-057)
/// a change that failed where no change was ever describable.
pub fn parse(locator: &str) -> Result<Position> {
    let normalised = normalize(locator);
    let refuse = |why: &str| Error::NotAPosition {
        locator: format!("{locator} ({why})"),
        normalised: normalised.clone(),
    };

    let (server, resource) = normalised.split_once(SERVER_SEPARATOR).ok_or_else(|| {
        refuse("an MCP locator names a server endpoint and a resource, separated by `#`")
    })?;
    if server.is_empty() {
        return Err(refuse("the server endpoint is empty"));
    }
    if resource.is_empty() {
        return Err(refuse("the resource URI is empty"));
    }
    if resource.contains(SERVER_SEPARATOR) {
        return Err(refuse(
            "the resource URI carries a `#` of its own, and this grammar cannot tell which one was \
             the separator (RFC 3986 §3.5 spends `#` on the fragment)",
        ));
    }
    if !server.contains(SCHEME_SEPARATOR) {
        return Err(refuse(
            "the server endpoint carries no scheme: a bare name is 'whichever server that name (sem: SEM-gx-adapter-mcp-058) \
             pointed at when this ran', and ASM-69-3 is the rule that a signed record may not (sem: SEM-gx-adapter-mcp-059) \
             depend on that",
        ));
    }
    Ok(Position {
        server: server.to_string(),
        resource: resource.to_string(),
    })
}

/// RFC 3986 §6.2.2's syntax-based normalisation, in the crate root's four clauses.
fn normalize_uri(uri: &str) -> String {
    let (scheme, rest) = match uri.split_once(SCHEME_SEPARATOR) {
        // Clause 1 (§3.1, §6.2.2.1).
        Some((scheme, rest)) => (Some(scheme.to_ascii_lowercase()), rest),
        None => (None, uri),
    };

    let (authority, path) = match scheme {
        // An authority exists only after a `://`. Everything up to the next `/`, `?` or `#` is it
        // (RFC 3986 §3.2); what follows is the path and is left to clauses 3-4.
        Some(_) => {
            let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            (Some(&rest[..end]), &rest[end..])
        }
        None => (None, rest),
    };

    let mut out = String::with_capacity(uri.len());
    if let Some(scheme) = &scheme {
        out.push_str(scheme);
        out.push_str(SCHEME_SEPARATOR);
    }
    if let Some(authority) = authority {
        out.push_str(&normalize_authority(authority));
    }
    out.push_str(&remove_dot_segments(&normalize_percent(path)));
    out
}

/// Clause 2 (§6.2.2.1): the **host** folds to lower case, and nothing else in the authority does.
///
/// The authority is `[userinfo "@"] host [":" port]`. Userinfo is credentials and is
/// case-**sensitive**; a port is digits. So the fold is applied between the last `@` and the port
/// separator, and the percent-encoding clause runs over the whole of it (a host may be
/// percent-encoded; §3.2.2 allows it).
fn normalize_authority(authority: &str) -> String {
    let authority = normalize_percent(authority);
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((userinfo, hostport)) => (Some(userinfo.to_string()), hostport.to_string()),
        None => (None, authority),
    };
    // An IPv6 literal is bracketed and may carry a `:` of its own (§3.2.2), so the port is looked for
    // after the closing bracket rather than at the first colon.
    let split = match hostport.rfind(']') {
        Some(bracket) => hostport[bracket..].find(':').map(|i| bracket + i),
        None => hostport.find(':'),
    };
    let (host, port) = match split {
        Some(i) => (&hostport[..i], &hostport[i..]),
        None => (hostport.as_str(), ""),
    };

    let mut out = String::with_capacity(hostport.len() + 1);
    if let Some(userinfo) = userinfo {
        out.push_str(&userinfo);
        out.push('@');
    }
    out.push_str(&host.to_ascii_lowercase());
    out.push_str(port);
    out
}

/// Clause 3 (§6.2.2.2): a percent triplet is spelled in upper case, and a triplet that encodes an
/// **unreserved** character is decoded.
///
/// "unreserved" is §2.3's set: `ALPHA / DIGIT / "-" / "." / "_" / "~"`. Decoding anything outside it (sem: SEM-gx-adapter-mcp-060)
/// would change what the URI means -- a `%2F` is not a `/` -- which is why the two halves of this
/// clause are not one.
///
/// A `%` that is not followed by two hexadecimal digits is left exactly as it is: it is not a triplet,
/// and rewriting it would be this function inventing a repair.
fn normalize_percent(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let (hi, lo) = (bytes[i + 1], bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hex_value(hi), hex_value(lo)) {
                let decoded = hi * 16 + lo;
                if is_unreserved(decoded) {
                    out.push(char::from(decoded));
                } else {
                    out.push('%');
                    out.push(char::from(hi_hex(hi)));
                    out.push(char::from(hi_hex(lo)));
                }
                i += 3;
                continue;
            }
        }
        // Not a triplet. Push the byte as part of whatever character it belongs to: the input is a
        // `&str`, so pushing byte by byte would split a multi-byte character.
        let rest = &text[i..];
        let c = rest
            .chars()
            .next()
            .expect("a non-empty remainder has a char");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hi_hex(value: u8) -> u8 {
    b"0123456789ABCDEF"[value as usize]
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// Clause 4 (§6.2.2.3, by the algorithm of §5.2.4): `.` and `..` are removed from the path.
///
/// The query is not a path and is left alone, so the algorithm runs over the text up to the first `?`.
///
/// 🔴 **An empty segment is not a dot segment.** `a//b` keeps both separators: §3.3 makes an empty
/// segment a segment, and a normaliser that collapsed them would merge two positions the way §6.2.3
/// would -- which the crate root refuses for the same reason. Only `.` and `..` are removed, and the
/// function returns its argument untouched when there are none, so the second pass of L7 has nothing
/// left to do.
fn remove_dot_segments(path: &str) -> String {
    let (path, tail) = match path.find('?') {
        Some(i) => (&path[..i], &path[i..]),
        None => (path, ""),
    };
    let absolute = path.starts_with('/');
    let body = if absolute { &path[1..] } else { path };
    if !body.split('/').any(|s| s == "." || s == "..") {
        return format!("{path}{tail}");
    }

    // A path ending in a dot segment ends in a separator once that segment is gone ("/a/." is "/a/"), (sem: SEM-gx-adapter-mcp-061)
    // so the trailing empty segment is written in before the split rather than patched in after.
    let body = if body == "." || body == ".." || body.ends_with("/.") || body.ends_with("/..") {
        format!("{body}/")
    } else {
        body.to_string()
    };

    let mut segments: Vec<&str> = Vec::new();
    for segment in body.split('/') {
        match segment {
            "." => {}
            ".." => {
                // "there is nothing above the root to name": a `..` that would climb out is dropped, (sem: SEM-gx-adapter-mcp-062)
                // which is §5.2.4's C step for a path already at its base.
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    let mut out = String::with_capacity(path.len() + tail.len());
    if absolute {
        out.push('/');
    }
    out.push_str(&segments.join("/"));
    out.push_str(tail);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L7's first half, at the level of the function rather than of the harness.
    #[test]
    fn normalising_twice_changes_nothing() {
        for spelling in [
            "HTTPS://MCP.Example:8443/sse#file:///srv/./a/../notes.md",
            "stdio://local#db://users/%7Ealice",
            "not a locator at all",
            "",
            "%zz#%7E",
        ] {
            let once = normalize(spelling);
            assert_eq!(normalize(&once), once, "{spelling:?}");
        }
    }

    /// One assertion per clause, so a broken clause names itself.
    #[test]
    fn each_clause_is_the_rfc_section_it_cites() {
        // 1: scheme (§3.1 / §6.2.2.1).
        assert_eq!(
            normalize("HTTPS://mcp.example/sse#file:///a"),
            "https://mcp.example/sse#file:///a"
        );
        // 2: host, and only the host (§6.2.2.1). The userinfo keeps its case.
        assert_eq!(
            normalize("https://Alice@MCP.Example:8443/sse#file:///a"),
            "https://Alice@mcp.example:8443/sse#file:///a"
        );
        // 3: triplet case, and the unreserved decode (§6.2.2.2). `%2F` is reserved and stays.
        assert_eq!(
            normalize("https://mcp.example/s#file:///a/%7eb%2fc%3A"),
            "https://mcp.example/s#file:///a/~b%2Fc%3A"
        );
        // 4: dot segments (§6.2.2.3 / §5.2.4).
        assert_eq!(
            normalize("https://mcp.example/s#file:///a/./b/../c"),
            "https://mcp.example/s#file:///a/c"
        );
    }

    /// 🔴 What is deliberately **not** normalised (§6.2.3), stated as a test so the residue is
    /// visible to a reader of the suite and not only to a reader of the crate root.
    #[test]
    fn scheme_based_normalisation_is_not_performed() {
        assert_ne!(
            normalize("https://mcp.example#file:///a"),
            normalize("https://mcp.example/#file:///a"),
            "an empty path and `/` are one URI for `http` (§6.2.3) and this adapter does not know \
             the scheme, so they are two positions and a policy author is writing on one of them"
        );
    }
}
