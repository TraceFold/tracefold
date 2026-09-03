// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use crate::manifest::DbManifest;
use crate::route::{self, Filters};
use crate::store;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};

pub const DEFAULT_PORT: u16 = 7423;
pub const ROUTES: [&str; 3] = ["/v1/ls", "/v1/show", "/v1/find"];

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
}

fn percent_decode(raw: &str) -> String {
    let bytes: Vec<u8> = raw.bytes().collect();
    let mut out: Vec<u8> = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let current = bytes[index];
        if current == b'+' {
            out.push(b' ');
            index += 1;
            continue;
        }
        if current == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) => {
                if header.trim().is_empty() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let (path, raw_query) = match target.split_once('?') {
        Some((path, rest)) => (path.to_string(), rest.to_string()),
        None => (target, String::new()),
    };
    let mut query: Vec<(String, String)> = Vec::new();
    for pair in raw_query.split('&') {
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((key, value)) => query.push((percent_decode(key), percent_decode(value))),
            None => query.push((percent_decode(pair), String::new())),
        }
    }
    Some(Request {
        method,
        path,
        query,
    })
}

fn param(request: &Request, name: &str) -> Option<String> {
    for (key, value) in &request.query {
        if key == name {
            return Some(value.clone());
        }
    }
    None
}

fn number(request: &Request, name: &str, fallback: usize) -> Result<usize, String> {
    match param(request, name) {
        None => Ok(fallback),
        Some(raw) => match raw.parse::<usize>() {
            Ok(value) => Ok(value),
            Err(_) => Err(format!(
                "{}={} is not a whole number; it is refused rather than replaced by {}",
                name, raw, fallback
            )),
        },
    }
}

fn refusal(reason: &str, detail: &str) -> String {
    let body = serde_json::json!({
        "schema": 1,
        "verdict": "UNKNOWN",
        "reason": reason,
        "exit": 2,
        "query": serde_json::Value::Null,
        "cap": serde_json::Value::Null,
        "denominator": { "matched": 0, "returned": 0, "unscanned": 0 },
        "rows": [],
        "note": detail,
    });
    match serde_json::to_string_pretty(&body) {
        Ok(text) => text,
        Err(_) => String::from("{\"schema\":1,\"verdict\":\"UNKNOWN\",\"reason\":\"WIRE_NOT_SERIALISED\"}"),
    }
}

fn answer(db: &Path, manifest_doc: &DbManifest, request: &Request) -> (u16, String) {
    if request.method != "GET" {
        return (
            405,
            refusal(
                "METHOD_NOT_READ_ONLY",
                "serve answers GET only; it never writes, so a method that implies a write is refused rather than ignored",
            ),
        );
    }
    if !ROUTES.contains(&request.path.as_str()) {
        return (
            404,
            refusal(
                "ROUTE_UNKNOWN",
                &format!("{} is not one of {}", request.path, ROUTES.join(" ")),
            ),
        );
    }
    let connection = match store::open_index(db) {
        Ok(connection) => connection,
        Err(error) => return (503, refusal("INDEX_ABSENT", &error)),
    };
    let lod = match number(request, "lod", if request.path == "/v1/show" { 1 } else { 0 }) {
        Ok(value) => value,
        Err(error) => return (400, refusal("UNKNOWN_FILTER_VALUE", &error)),
    };
    let outcome = match request.path.as_str() {
        "/v1/ls" => route::ls(
            &connection,
            manifest_doc,
            &Filters {
                band: param(request, "band"),
                layer: param(request, "layer"),
                role: param(request, "role"),
                executor: param(request, "executor"),
            },
            lod,
            param(request, "cursor").as_deref(),
        ),
        "/v1/show" => match param(request, "address") {
            Some(address) => route::show(&connection, &address, lod),
            None => {
                return (
                    400,
                    refusal("ADDRESS_ABSENT", "/v1/show needs address=<id or exact address>"),
                )
            }
        },
        _ => {
            let limit = match number(request, "limit", 10) {
                Ok(value) => value,
                Err(error) => return (400, refusal("UNKNOWN_FILTER_VALUE", &error)),
            };
            match param(request, "needle") {
                Some(needle) => route::find(
                    &connection,
                    manifest_doc,
                    &needle,
                    &Filters {
                        band: param(request, "band"),
                        layer: param(request, "layer"),
                        role: None,
                        executor: None,
                    },
                    limit,
                ),
                None => return (400, refusal("NEEDLE_ABSENT", "/v1/find needs needle=<text>")),
            }
        }
    };
    let status = if outcome.exit == 0 { 200 } else { 422 };
    (status, route::wire(&outcome))
}

fn respond(mut stream: TcpStream, status: u16, body: String) {
    let phrase = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        422 => "Unprocessable Content",
        _ => "Service Unavailable",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        phrase,
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

fn accept_forever(listener: TcpListener, db: PathBuf, manifest_doc: DbManifest) {
    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        match parse_request(&stream) {
            Some(request) => {
                let (status, body) = answer(&db, &manifest_doc, &request);
                respond(stream, status, body);
            }
            None => respond(stream, 400, refusal("REQUEST_UNREADABLE", "the request line could not be read")),
        }
    }
}

pub fn serve(db: &Path, manifest_doc: &DbManifest, port: u16) -> i32 {
    let loopback_v4 = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let loopback_v6 = SocketAddr::from((Ipv6Addr::LOCALHOST, port));
    let four = match TcpListener::bind(loopback_v4) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "UNTESTABLE: {} could not be bound: {}. Nothing is serving, and a port that is not listening is not a server that returns no rows",
                loopback_v4, error
            );
            return 2;
        }
    };
    match TcpListener::bind(loopback_v6) {
        Ok(listener) => {
            println!("listening on http://{} and http://[{}]:{}", loopback_v4, Ipv6Addr::LOCALHOST, port);
            println!(
                "both loopbacks are bound on purpose: a browser resolves localhost to ::1 first, so a server bound only to 127.0.0.1 answers ERR_CONNECTION_REFUSED to http://localhost:{}",
                port
            );
            let db = db.to_path_buf();
            let carried = manifest_doc.clone();
            std::thread::spawn(move || accept_forever(listener, db, carried));
        }
        Err(error) => {
            println!("listening on http://{} only", loopback_v4);
            println!(
                "[{}]:{} could not be bound ({}), so http://localhost:{} will be refused by a browser that resolves localhost to ::1; use http://127.0.0.1:{}",
                Ipv6Addr::LOCALHOST,
                port,
                error,
                port,
                port
            );
        }
    }
    println!("routes: {} (GET only, read only, one serialiser shared with --json on stdout)", ROUTES.join(" "));
    accept_forever(four, db.to_path_buf(), manifest_doc.clone());
    0
}
