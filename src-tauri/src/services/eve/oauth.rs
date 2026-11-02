use super::{Error, Secret, CLIENT_ID, REDIRECT_URI, SCOPES};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};

pub(super) fn random() -> Result<Secret, Error> {
    let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
    getrandom::fill(bytes.as_mut()).map_err(|_| Error::Unavailable)?;
    Ok(zeroize::Zeroizing::new(
        URL_SAFE_NO_PAD.encode(bytes.as_ref()),
    ))
}
pub(super) fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
pub(super) fn authorize_url(state: &str, verifier: &str) -> Result<reqwest::Url, Error> {
    if state.len() != 43 || verifier.len() != 43 {
        return Err(Error::Authentication);
    }
    let mut url = reqwest::Url::parse(super::AUTHORIZE).map_err(|_| Error::Authentication)?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("scope", SCOPES),
        ("state", state),
        ("code_challenge_method", "S256"),
        ("code_challenge", &challenge(verifier)),
    ]);
    Ok(url)
}

// Only public URL construction reaches the OS browser; no shell command, renderer
// supplied URL, embedded WebView, auth code, bearer or refresh token is passed.
pub(super) fn browser(url: &reqwest::Url) -> Result<(), Error> {
    if url.scheme() != "https"
        || url.host_str() != Some("login.eveonline.com")
        || url.path() != "/v2/oauth/authorize"
    {
        return Err(Error::Authentication);
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                window: *mut std::ffi::c_void,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show: i32,
            ) -> *mut std::ffi::c_void;
        }
        let action: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        let path: Vec<u16> = std::ffi::OsStr::new(url.as_str())
            .encode_wide()
            .chain(Some(0))
            .collect();
        // NUL-terminated owned buffers; no parameters or command-line interpolation.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                action.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as usize <= 32 {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        Err(Error::Unsupported)
    }
}

pub(super) fn callback(request: &[u8], expected: &str) -> Result<Secret, Error> {
    if request.len() > 8192 || !request.ends_with(b"\r\n\r\n") {
        return Err(Error::Authentication);
    }
    let text = std::str::from_utf8(request).map_err(|_| Error::Authentication)?;
    let mut lines = text.split("\r\n");
    let line = lines.next().ok_or(Error::Authentication)?;
    let parts: Vec<_> = line.split(' ').collect();
    if parts.len() != 3 || parts[0] != "GET" || parts[2] != "HTTP/1.1" {
        return Err(Error::Authentication);
    }
    let mut host = None;
    for line in lines.take_while(|s| !s.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(Error::Authentication)?;
        if line.starts_with([' ', '\t']) {
            return Err(Error::Authentication);
        }
        if name.eq_ignore_ascii_case("host") && host.replace(value.trim()).is_some() {
            return Err(Error::Authentication);
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            || (name.eq_ignore_ascii_case("content-length") && value.trim() != "0")
        {
            return Err(Error::Authentication);
        }
    }
    if host != Some("localhost:43173")
        || parts[1].split('?').next() != Some("/oauth/callback")
        || parts[1].contains('#')
    {
        return Err(Error::Authentication);
    }
    let url = reqwest::Url::parse(&format!("http://localhost:43173{}", parts[1]))
        .map_err(|_| Error::Authentication)?;
    let mut values = BTreeMap::new();
    for (key, value) in url.query_pairs() {
        if !["state", "code", "error", "error_description"].contains(&key.as_ref())
            || value.len() > 2048
            || values
                .insert(
                    key.into_owned(),
                    zeroize::Zeroizing::new(value.into_owned()),
                )
                .is_some()
        {
            return Err(Error::Authentication);
        }
    }
    let state = values.remove("state").ok_or(Error::Authentication)?;
    // Both states are uniformly random and fixed length. Constant-time byte compare.
    if state.len() != expected.len()
        || state
            .as_bytes()
            .iter()
            .zip(expected.as_bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            != 0
    {
        return Err(Error::Authentication);
    }
    if values.contains_key("error") {
        if values.contains_key("code") {
            return Err(Error::Authentication);
        }
        return Err(Error::Denied);
    }
    let code = values.remove("code").ok_or(Error::Authentication)?;
    if code.is_empty() {
        return Err(Error::Authentication);
    }
    Ok(code)
}

pub(super) fn receive(
    listener: TcpListener,
    state: &str,
    cancelled: impl Fn() -> bool,
    timeout: Duration,
) -> Result<Secret, Error> {
    listener
        .set_nonblocking(true)
        .map_err(|_| Error::Unavailable)?;
    let deadline = Instant::now() + timeout;
    let mut attempts = 0;
    while Instant::now() < deadline {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                attempts += 1;
                let result = read_callback(&mut stream, state);
                let accepted = result.is_ok() || matches!(result, Err(Error::Denied));
                let status = if accepted {
                    "200 OK"
                } else {
                    "400 Bad Request"
                };
                let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\nReturn to ECHELON OS. This window can be closed.");
                if accepted {
                    return result;
                }
                if attempts >= 16 {
                    return Err(Error::Authentication);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25))
            }
            Err(_) => return Err(Error::Unavailable),
        }
    }
    Err(Error::Timeout)
}
fn read_callback(stream: &mut TcpStream, state: &str) -> Result<Secret, Error> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| Error::Unavailable)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| Error::Unavailable)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut request = zeroize::Zeroizing::new(Vec::new());
    let mut chunk = zeroize::Zeroizing::new([0u8; 512]);
    while request.len() < 8192 {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::Authentication)?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| Error::Unavailable)?;
        let n = stream
            .read(chunk.as_mut())
            .map_err(|_| Error::Authentication)?;
        if n == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..n]);
        if request.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    callback(&request, state)
}
