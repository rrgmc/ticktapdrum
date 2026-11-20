//! Downloads a groove file from a link a person pasted or shared.
//!
//! The download blocks, so the interface runs it on a worker thread. It only fetches bytes:
//! [`crate::import`] decides from the content whether they are grooves. A web page is refused
//! rather than searched for links, as `docs/DESIGN.md` ("Milestone 13: add from a link") says.

use std::io::{self, Read};
use std::time::Duration;

use ureq::ResponseExt;

use crate::MAX_INPUT_BYTES;

/// Sent with every request. It names the app and nothing else.
const USER_AGENT: &str = "TickTapDrum";
/// The most time for reaching the server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// The most time for the whole download, redirects and body included.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
/// The bytes read at a time.
const CHUNK: usize = 64 * 1024;
/// How many bytes to wait for before checking whether the file is a web page. The start of a
/// page is always in its first kilobyte.
const SNIFF_BYTES: usize = 1024;
/// The name used when neither the server nor the address gives one.
const FALLBACK_NAME: &str = "download";

/// Why a link could not be downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Not an http or https address.
    NotALink,
    /// The server could not be reached, or the download failed or timed out. The text says why,
    /// for the log.
    Failed(String),
    /// The server answered with an error status (404 and so on).
    Status(u16),
    /// Larger than [`MAX_INPUT_BYTES`].
    TooLarge,
    /// A web page (HTML), not a groove file.
    WebPage,
}

/// The address a person pasted or shared, cleaned up: trimmed, and a GitHub `.../blob/...` page
/// address changed to its raw file (`https://github.com/u/r/blob/b/p` becomes
/// `https://raw.githubusercontent.com/u/r/b/p`). `None` if it is not an http or https address.
pub fn link_address(text: &str) -> Option<String> {
    let text = text.trim();
    let rest = strip_prefix_ignore_case(text, "https://")
        .or_else(|| strip_prefix_ignore_case(text, "http://"))?;
    // A link has a host, and pasted text with spaces in it is not one link.
    if rest.is_empty() || rest.starts_with('/') || text.chars().any(char::is_whitespace) {
        return None;
    }
    Some(github_raw(rest).unwrap_or_else(|| text.to_owned()))
}

/// Downloads a link for [`crate::import`] and returns the file's name and its bytes.
///
/// The name comes from the server's `Content-Disposition` header if it gives one. Otherwise it is
/// the last part of the final address, after redirects, percent-decoded. If neither gives a name,
/// it is `download`.
///
/// `progress` is called now and then with the bytes so far and the total, if the server says.
/// Returning `false` from it cancels the download, which then fails with `Failed("cancelled")`.
pub fn download(
    link: &str,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(String, Vec<u8>), LinkError> {
    let address = link_address(link).ok_or(LinkError::NotALink)?;
    let cancelled = || LinkError::Failed("cancelled".to_owned());

    let response = agent().get(&address).call().map_err(|e| match e {
        ureq::Error::StatusCode(code) => LinkError::Status(code),
        other => LinkError::Failed(other.to_string()),
    })?;

    let total = response.body().content_length();
    if total.is_some_and(|len| len > MAX_INPUT_BYTES as u64) {
        return Err(LinkError::TooLarge);
    }
    if response
        .body()
        .mime_type()
        .is_some_and(|mime| mime.eq_ignore_ascii_case("text/html"))
    {
        return Err(LinkError::WebPage);
    }
    let disposition = response
        .headers()
        .get("content-disposition")
        .and_then(|value| value.to_str().ok())
        .and_then(disposition_name);
    let name = disposition
        .or_else(|| path_name(response.get_uri().path()))
        .unwrap_or_else(|| FALLBACK_NAME.to_owned());

    if !progress(0, total) {
        return Err(cancelled());
    }
    // One byte more than the limit is enough to know the file is too large.
    let mut body = response
        .into_body()
        .into_reader()
        .take(MAX_INPUT_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    let mut buf = vec![0u8; CHUNK];
    let mut sniffed = false;
    let mut reported = 0usize;
    loop {
        let n = match body.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(LinkError::Failed(e.to_string())),
        };
        bytes.extend_from_slice(&buf[..n]);
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(LinkError::TooLarge);
        }
        if !sniffed && bytes.len() >= SNIFF_BYTES {
            sniffed = true;
            if looks_like_html(&bytes) {
                return Err(LinkError::WebPage);
            }
        }
        if bytes.len() - reported >= CHUNK {
            reported = bytes.len();
            if !progress(reported as u64, total) {
                return Err(cancelled());
            }
        }
    }
    if !sniffed && looks_like_html(&bytes) {
        return Err(LinkError::WebPage);
    }
    if !progress(bytes.len() as u64, total) {
        return Err(cancelled());
    }
    Ok((name, bytes))
}

/// The HTTP client. Status errors come back as `ureq::Error::StatusCode`, and redirects are
/// followed (ureq's default of ten at most).
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .build()
        .into()
}

/// `text` without `prefix`, comparing ASCII letters without regard to case.
fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// The raw file's address for a GitHub file page, given the address without its scheme. `None`
/// for any other address.
fn github_raw(rest: &str) -> Option<String> {
    let split = rest.find(['?', '#']).unwrap_or(rest.len());
    let (path, tail) = rest.split_at(split);
    let mut parts = path.splitn(5, '/');
    let host = parts.next()?;
    let (user, repo, blob, file) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let host_ok =
        host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("www.github.com");
    // `file` is the branch and the path inside the repository, so it needs a `/` too.
    let file_ok = file
        .split_once('/')
        .is_some_and(|(branch, path)| !branch.is_empty() && !path.is_empty());
    if !host_ok || user.is_empty() || repo.is_empty() || blob != "blob" || !file_ok {
        return None;
    }
    Some(format!(
        "https://raw.githubusercontent.com/{user}/{repo}/{file}{tail}"
    ))
}

/// The file name a `Content-Disposition` header gives, if any. An RFC 5987 `filename*` wins over
/// a plain `filename`.
fn disposition_name(header: &str) -> Option<String> {
    let mut plain = None;
    let mut extended = None;
    for param in header.split(';').skip(1) {
        let Some((key, value)) = param.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.eq_ignore_ascii_case("filename*") {
            // The form is `charset'language'percent-encoded-name`.
            let encoded = value.splitn(3, '\'').nth(2).unwrap_or(value);
            extended = Some(percent_decode(encoded));
        } else if key.eq_ignore_ascii_case("filename") {
            plain = Some(unquote(value));
        }
    }
    extended
        .and_then(|name| base_name(&name))
        .or_else(|| plain.and_then(|name| base_name(&name)))
}

/// A header value without its surrounding quotes, with `\"` and `\\` unescaped.
fn unquote(value: &str) -> String {
    let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return value.to_owned();
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The file name at the end of an address's path, percent-decoded.
fn path_name(path: &str) -> Option<String> {
    let last = path.rsplit('/').next().unwrap_or(path);
    base_name(&percent_decode(last))
}

/// The last part of a name that may hold folders, or `None` if nothing usable is left.
fn base_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    if base.is_empty() || base == "." || base == ".." {
        return None;
    }
    Some(base.to_owned())
}

/// Text with `%XX` escapes turned back into bytes. Bytes that are not UTF-8 become `U+FFFD`.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether the bytes start like an HTML page: `<!doctype html` or `<html`, in any case, after any
/// blank space and a UTF-8 byte order mark.
fn looks_like_html(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let bytes = &bytes[start..];
    [b"<!doctype html".as_slice(), b"<html".as_slice()]
        .iter()
        .any(|tag| {
            bytes
                .get(..tag.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(tag))
        })
}

#[cfg(test)]
mod tests;
