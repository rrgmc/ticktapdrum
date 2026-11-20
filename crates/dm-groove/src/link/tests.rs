use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use super::*;

// ---- link_address ----

#[test]
fn address_is_trimmed() {
    assert_eq!(
        link_address("  https://example.com/a.mid \n").as_deref(),
        Some("https://example.com/a.mid")
    );
}

#[test]
fn http_and_any_case_scheme_are_links() {
    assert_eq!(
        link_address("http://example.com/a.zip").as_deref(),
        Some("http://example.com/a.zip")
    );
    assert_eq!(
        link_address("HTTPS://example.com/a.zip").as_deref(),
        Some("HTTPS://example.com/a.zip")
    );
}

#[test]
fn query_string_is_kept() {
    assert_eq!(
        link_address("https://example.com/get?id=7&format=zip").as_deref(),
        Some("https://example.com/get?id=7&format=zip")
    );
}

#[test]
fn github_file_page_becomes_raw_file() {
    assert_eq!(
        link_address("https://github.com/user/repo/blob/main/packs/Rock%201.mid").as_deref(),
        Some("https://raw.githubusercontent.com/user/repo/main/packs/Rock%201.mid")
    );
    assert_eq!(
        link_address("http://www.github.com/u/r/blob/b/p.zip?raw=true").as_deref(),
        Some("https://raw.githubusercontent.com/u/r/b/p.zip?raw=true")
    );
}

#[test]
fn other_github_pages_are_left_alone() {
    for link in [
        "https://github.com/user/repo",
        "https://github.com/user/repo/blob/main",
        "https://github.com/user/repo/tree/main/packs",
        "https://github.com/user/repo/releases/download/v1/pack.zip",
    ] {
        assert_eq!(link_address(link).as_deref(), Some(link));
    }
}

#[test]
fn non_http_text_is_not_a_link() {
    for text in [
        "",
        "   ",
        "www.example.com/a.mid",
        "example.com/a.mid",
        "ftp://example.com/a.mid",
        "file:///C:/a.mid",
        "https://",
        "https:///a.mid",
        "https://example.com/a.mid and more",
    ] {
        assert_eq!(link_address(text), None, "{text:?}");
    }
}

// ---- helpers ----

#[test]
fn html_is_recognized() {
    assert!(looks_like_html(b"<!DOCTYPE html><html></html>"));
    assert!(looks_like_html(b"\r\n  <!doctype HTML>"));
    assert!(looks_like_html(b"\xEF\xBB\xBF<html lang=\"en\">"));
    assert!(looks_like_html(b"<HTML>"));
    assert!(!looks_like_html(b"MThd\0\0\0\x06"));
    assert!(!looks_like_html(b"PK\x03\x04"));
    assert!(!looks_like_html(b"<?xml version=\"1.0\"?>"));
    assert!(!looks_like_html(b"<htm"));
    assert!(!looks_like_html(b""));
}

#[test]
fn name_from_path() {
    assert_eq!(
        path_name("/packs/Rock%20Beat%201.mid").as_deref(),
        Some("Rock Beat 1.mid")
    );
    assert_eq!(path_name("/caf%C3%A9.zip").as_deref(), Some("café.zip"));
    assert_eq!(path_name("/a%2Fb%5Cc.mid").as_deref(), Some("c.mid"));
    assert_eq!(path_name("/100%.mid").as_deref(), Some("100%.mid"));
    assert_eq!(path_name("/packs/"), None);
    assert_eq!(path_name("/"), None);
    assert_eq!(path_name("/.."), None);
}

#[test]
fn name_from_content_disposition() {
    assert_eq!(
        disposition_name("attachment; filename=\"Funk Pack.zip\"").as_deref(),
        Some("Funk Pack.zip")
    );
    assert_eq!(
        disposition_name("attachment; filename=pack.zip").as_deref(),
        Some("pack.zip")
    );
    assert_eq!(
        disposition_name("attachment; filename=\"a.zip\"; filename*=UTF-8''caf%C3%A9.zip")
            .as_deref(),
        Some("café.zip")
    );
    assert_eq!(
        disposition_name("attachment; FILENAME=\"../../x.mid\"").as_deref(),
        Some("x.mid")
    );
    assert_eq!(
        disposition_name("attachment; filename=C:\\packs\\x.mid").as_deref(),
        Some("x.mid")
    );
    assert_eq!(
        disposition_name("attachment; filename=\"say \\\"hi\\\".mid\"").as_deref(),
        Some("say \"hi\".mid")
    );
    assert_eq!(disposition_name("inline"), None);
    assert_eq!(disposition_name("attachment; filename=\"\""), None);
}

// ---- download, against a local server ----

/// A response the test server sends, given the request's path.
type Handler = Box<dyn Fn(&str, &mut TcpStream) + Send>;

/// Starts a server on a free local port that answers `connections` requests with `handler`, and
/// returns its base address.
fn serve(connections: usize, handler: Handler) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming().take(connections) {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_owned();
            // Skip the headers.
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) && line != "\r\n" {
                line.clear();
            }
            handler(&path, &mut stream);
        }
    });
    base
}

/// Writes a whole response with a `Content-Length`. Errors are ignored: the client may hang up.
fn respond(stream: &mut TcpStream, status: &str, headers: &[&str], body: &[u8]) {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for header in headers {
        head.push_str(header);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

/// A progress callback that never cancels.
fn keep_going() -> impl FnMut(u64, Option<u64>) -> bool {
    |_, _| true
}

#[test]
fn not_a_link_is_refused_before_any_request() {
    assert_eq!(
        download("www.example.com/a.mid", &mut keep_going()),
        Err(LinkError::NotALink)
    );
}

#[test]
fn downloads_a_file_with_its_name_and_progress() {
    let base = serve(
        1,
        Box::new(|_, s| respond(s, "200 OK", &["Content-Type: audio/midi"], b"MThd data")),
    );
    let mut calls = Vec::new();
    let result = download(&format!("{base}/packs/Rock%201.mid"), &mut |done, total| {
        calls.push((done, total));
        true
    });
    assert_eq!(result, Ok(("Rock 1.mid".to_owned(), b"MThd data".to_vec())));
    assert_eq!(calls.first(), Some(&(0, Some(9))));
    assert_eq!(calls.last(), Some(&(9, Some(9))));
}

#[test]
fn content_disposition_names_the_file() {
    let base = serve(
        1,
        Box::new(|_, s| {
            respond(
                s,
                "200 OK",
                &["Content-Disposition: attachment; filename=\"Funk Pack.zip\""],
                b"PK\x03\x04",
            )
        }),
    );
    let (name, _) = download(&format!("{base}/get?id=3"), &mut keep_going()).unwrap();
    assert_eq!(name, "Funk Pack.zip");
}

#[test]
fn nameless_address_is_named_download() {
    let base = serve(1, Box::new(|_, s| respond(s, "200 OK", &[], b"MThd")));
    let (name, _) = download(&format!("{base}/"), &mut keep_going()).unwrap();
    assert_eq!(name, "download");
}

#[test]
fn redirects_are_followed_and_name_the_file() {
    let base = serve(
        2,
        Box::new(|path, s| {
            if path == "/start" {
                respond(s, "302 Found", &["Location: /files/final.zip"], b"");
            } else {
                respond(s, "200 OK", &[], b"PK\x03\x04");
            }
        }),
    );
    let (name, bytes) = download(&format!("{base}/start"), &mut keep_going()).unwrap();
    assert_eq!(name, "final.zip");
    assert_eq!(bytes, b"PK\x03\x04");
}

#[test]
fn error_status_is_reported() {
    let base = serve(
        1,
        Box::new(|_, s| respond(s, "404 Not Found", &[], b"gone")),
    );
    assert_eq!(
        download(&format!("{base}/a.mid"), &mut keep_going()),
        Err(LinkError::Status(404))
    );
}

#[test]
fn html_content_type_is_a_web_page() {
    let base = serve(
        1,
        Box::new(|_, s| {
            respond(
                s,
                "200 OK",
                &["Content-Type: text/html; charset=utf-8"],
                b"hello",
            )
        }),
    );
    assert_eq!(
        download(&format!("{base}/a.mid"), &mut keep_going()),
        Err(LinkError::WebPage)
    );
}

#[test]
fn html_content_is_a_web_page() {
    let base = serve(
        1,
        Box::new(|_, s| {
            respond(
                s,
                "200 OK",
                &["Content-Type: application/octet-stream"],
                b"\n<!DOCTYPE html><title>Pack</title>",
            )
        }),
    );
    assert_eq!(
        download(&format!("{base}/a.zip"), &mut keep_going()),
        Err(LinkError::WebPage)
    );
}

#[test]
fn large_content_length_is_refused_early() {
    let base = serve(
        1,
        Box::new(|_, s| {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_INPUT_BYTES + 1
            );
            let _ = s.write_all(head.as_bytes());
        }),
    );
    assert_eq!(
        download(&format!("{base}/big.zip"), &mut keep_going()),
        Err(LinkError::TooLarge)
    );
}

#[test]
fn large_body_without_length_is_stopped() {
    let base = serve(
        1,
        Box::new(|_, s| {
            // No Content-Length: the body runs until the server closes the connection.
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
            let chunk = vec![0u8; 1024 * 1024];
            for _ in 0..=MAX_INPUT_BYTES / chunk.len() {
                if s.write_all(&chunk).is_err() {
                    return;
                }
            }
        }),
    );
    assert_eq!(
        download(&format!("{base}/big.zip"), &mut keep_going()),
        Err(LinkError::TooLarge)
    );
}

#[test]
fn progress_can_cancel() {
    let base = serve(1, Box::new(|_, s| respond(s, "200 OK", &[], b"MThd")));
    assert_eq!(
        download(&format!("{base}/a.mid"), &mut |_, _| false),
        Err(LinkError::Failed("cancelled".to_owned()))
    );
}

#[test]
fn unreachable_server_fails() {
    // Bind and drop a listener so the port is very likely closed.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert!(matches!(
        download(&format!("http://127.0.0.1:{port}/a.mid"), &mut keep_going()),
        Err(LinkError::Failed(_))
    ));
}
