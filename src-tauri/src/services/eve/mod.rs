//! Session-only, explicit EVE SSO/ESI boundary. No disk writes, token IPC or polling.
mod http;
mod oauth;
#[cfg(test)]
mod tests;
use http::{CcpHttp, Request, Response, Target, Transport};
use serde::{Deserialize, Serialize};
use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::{Zeroize, Zeroizing};

type Secret = Zeroizing<String>;
pub const CLIENT_ID: &str = "49be7fb9afec4015a78f6034d126db8e";
pub const REDIRECT_URI: &str = "http://localhost:43173/oauth/callback";
const SCOPES: &str = "esi-location.read_location.v1 esi-ui.write_waypoint.v1";
const COMPATIBILITY_DATE: &str = "2026-08-18";
const DISCOVERY: &str = "https://login.eveonline.com/.well-known/oauth-authorization-server";
const AUTHORIZE: &str = "https://login.eveonline.com/v2/oauth/authorize";
const TOKEN: &str = "https://login.eveonline.com/v2/oauth/token";
const JWKS: &str = "https://login.eveonline.com/oauth/jwks";
const MAX_ID: u64 = 9_007_199_254_740_991;
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    Unavailable,
    Unsupported,
    Busy,
    PortOccupied,
    Timeout,
    Authentication,
    Denied,
    Cancelled,
    InvalidResponse,
    Disconnected,
    Expired,
    Forbidden,
    RateLimited,
    InvalidDestination,
    InvalidConfirmation,
    OutcomeUnknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Disconnected,
    Connecting,
    Connected,
    Expired,
    Error,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub id: u64,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub system_id: u64,
    pub fetched_at: u64,
    pub server_as_of: Option<u64>,
    pub fresh_until: u64,
    pub cached: bool,
    pub stale: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub phase: Phase,
    pub character: Option<Character>,
    pub location: Option<Location>,
    pub error: Option<Error>,
    pub retry_at: Option<u64>,
    pub expires_at: Option<u64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    SetDestination,
    AddWaypoint,
}
#[derive(Clone, Serialize)]
pub struct Confirmation {
    pub id: String,
    pub character: Character,
    pub destination: u64,
    pub action: Action,
    pub expires_at: u64,
}
struct Session {
    access: Secret,
    character: Character,
    expires: u64,
    location: Option<Location>,
    etag: Option<String>,
    // Dropping the session wakes its expiry worker immediately, including logout.
    _expiry_cancel: Option<std::sync::mpsc::Sender<()>>,
}
struct State {
    generation: u64,
    phase: Phase,
    session: Option<Session>,
    confirmation: Option<Confirmation>,
    error: Option<Error>,
    blocked_until: u64,
}
struct Inner {
    state: Mutex<State>,
    busy: AtomicBool,
    http: Arc<dyn Transport>,
}
#[derive(Clone)]
pub struct Eve {
    inner: Arc<Inner>,
}
struct Flight<'a>(&'a AtomicBool);
impl Drop for Flight<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl Default for Eve {
    fn default() -> Self {
        Self::new(Arc::new(CcpHttp))
    }
}
impl Eve {
    /// Even an ACL regression in native dispatch tests must never contact CCP.
    #[cfg(test)]
    pub fn offline_for_acl_tests() -> Self {
        struct Refuse;
        impl Transport for Refuse {
            fn send(&self, _: Request) -> Result<Response, Error> {
                Err(Error::Unsupported)
            }
        }
        Self::new(Arc::new(Refuse))
    }
    fn new(http: Arc<dyn Transport>) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    generation: 0,
                    phase: Phase::Disconnected,
                    session: None,
                    confirmation: None,
                    error: None,
                    blocked_until: 0,
                }),
                busy: AtomicBool::new(false),
                http,
            }),
        }
    }
    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>, Error> {
        self.inner.state.lock().map_err(|_| Error::Unavailable)
    }
    fn flight(&self) -> Result<Flight<'_>, Error> {
        self.inner
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Error::Busy)?;
        Ok(Flight(&self.inner.busy))
    }
    fn current(&self, generation: u64) -> bool {
        self.state().is_ok_and(|s| s.generation == generation)
    }
    pub fn status(&self) -> Result<View, Error> {
        let mut s = self.state()?;
        if s.session.as_ref().is_some_and(|v| v.expires <= now()) {
            s.session = None;
            s.confirmation = None;
            s.phase = Phase::Expired;
            s.error = Some(Error::Expired);
        }
        let mut location = s.session.as_ref().and_then(|v| v.location.clone());
        if let Some(l) = &mut location {
            l.stale = now() >= l.fresh_until;
        }
        Ok(View {
            phase: s.phase,
            expires_at: s.session.as_ref().map(|v| v.expires),
            character: s.session.as_ref().map(|v| v.character.clone()),
            location,
            error: s.error,
            retry_at: (s.blocked_until > now()).then_some(s.blocked_until),
        })
    }
    pub fn disconnect(&self) -> Result<View, Error> {
        {
            let mut s = self.state()?;
            s.generation = s.generation.wrapping_add(1);
            s.session = None;
            s.confirmation = None;
            s.phase = Phase::Disconnected;
            s.error = None;
        }
        self.status()
    }
    fn fail(&self, generation: u64, error: Error) {
        if let Ok(mut s) = self.state() {
            if s.generation == generation {
                s.error = Some(error);
                if matches!(
                    error,
                    Error::Expired | Error::Authentication | Error::Forbidden
                ) {
                    s.session = None;
                    s.confirmation = None;
                    s.phase = if error == Error::Expired {
                        Phase::Expired
                    } else {
                        Phase::Error
                    };
                } else if s.phase == Phase::Connecting {
                    s.phase = Phase::Error;
                }
            }
        }
    }
    pub fn connect(&self) -> Result<View, Error> {
        self.connect_with(oauth::browser)
    }
    fn connect_with(
        &self,
        open_browser: impl FnOnce(&reqwest::Url) -> Result<(), Error>,
    ) -> Result<View, Error> {
        let _flight = self.flight()?;
        let generation = {
            let mut s = self.state()?;
            if s.blocked_until > now() {
                return Err(Error::RateLimited);
            }
            if s.phase == Phase::Connected {
                return Err(Error::Busy);
            }
            s.generation = s.generation.wrapping_add(1);
            s.session = None;
            s.confirmation = None;
            s.phase = Phase::Connecting;
            s.error = None;
            s.generation
        };
        let result = (|| {
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 43173))
                .map_err(|_| Error::PortOccupied)?;
            let state = oauth::random()?;
            let verifier = oauth::random()?;
            let discovery = self.inner.http.send(Request {
                target: Target::Discovery,
                bearer: None,
            })?;
            self.limits(generation, &discovery);
            if discovery.status != 200 {
                return Err(if discovery.status == 429 {
                    Error::RateLimited
                } else if discovery.status >= 500 {
                    Error::Unavailable
                } else {
                    Error::Authentication
                });
            }
            validate_discovery(&discovery.body)?;
            if !self.current(generation) {
                return Err(Error::Cancelled);
            }
            open_browser(&oauth::authorize_url(&state, &verifier)?)?;
            let code = oauth::receive(
                listener,
                &state,
                || !self.current(generation),
                Duration::from_secs(120),
            )?;
            if !self.current(generation) {
                return Err(Error::Cancelled);
            }
            let response = self.inner.http.send(Request {
                target: Target::Token { code, verifier },
                bearer: None,
            })?;
            self.limits(generation, &response);
            if response.status != 200 {
                return Err(if response.status == 429 {
                    Error::RateLimited
                } else if response.status >= 500 {
                    Error::Unavailable
                } else {
                    Error::Authentication
                });
            }
            let token: TokenReply =
                serde_json::from_slice(&response.body).map_err(|_| Error::Authentication)?;
            if !token.token_type.eq_ignore_ascii_case("bearer")
                || token.expires_in == 0
                || token.expires_in > 86400
            {
                return Err(Error::Authentication);
            }
            let keys = self.inner.http.send(Request {
                target: Target::Jwks,
                bearer: None,
            })?;
            self.limits(generation, &keys);
            if keys.status != 200 {
                return Err(if keys.status == 429 {
                    Error::RateLimited
                } else if keys.status >= 500 {
                    Error::Unavailable
                } else {
                    Error::Authentication
                });
            }
            let claims = verify(&token.access_token, &keys.body, now())?;
            let expires = claims.exp.min(now().saturating_add(token.expires_in));
            let (expiry_cancel, expiry_wait) = std::sync::mpsc::channel();
            {
                let mut s = self.state()?;
                if s.generation != generation {
                    return Err(Error::Cancelled);
                }
                s.session = Some(Session {
                    _expiry_cancel: Some(expiry_cancel),
                    access: Zeroizing::new(token.access_token.clone()),
                    character: Character {
                        id: claims.character_id()?,
                        name: claims.name,
                    },
                    expires,
                    location: None,
                    etag: None,
                });
                s.phase = Phase::Connected;
                s.error = None;
            }
            // A one-shot local expiry timer, no background HTTP/polling. Weak ownership
            // doesn't retain the session after app state is dropped. Disconnect clears it now.
            let weak = Arc::downgrade(&self.inner);
            std::thread::spawn(move || {
                if !matches!(
                    expiry_wait.recv_timeout(Duration::from_secs(expires.saturating_sub(now()))),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ) {
                    return;
                }
                if let Some(inner) = weak.upgrade() {
                    if let Ok(mut s) = inner.state.lock() {
                        if s.generation == generation
                            && s.session.as_ref().is_some_and(|v| v.expires <= now())
                        {
                            s.session = None;
                            s.confirmation = None;
                            s.phase = Phase::Expired;
                            s.error = Some(Error::Expired);
                        }
                    }
                }
            });
            self.status()
        })();
        if let Err(e) = result {
            self.fail(generation, e);
        }
        result
    }
    fn credentials(&self) -> Result<(u64, Secret, u64), Error> {
        let _ = self.status()?;
        let s = self.state()?;
        if s.blocked_until > now() {
            return Err(Error::RateLimited);
        }
        let session = s.session.as_ref().ok_or(if s.phase == Phase::Expired {
            Error::Expired
        } else {
            Error::Disconnected
        })?;
        Ok((s.generation, session.access.clone(), session.character.id))
    }
    fn limits(&self, generation: u64, response: &Response) {
        let at = now();
        let delay = response.headers.get("retry-after").and_then(|x| {
            x.parse::<u64>().ok().or_else(|| {
                httpdate::parse_http_date(x)
                    .ok()
                    .and_then(|d| d.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().saturating_sub(at))
            })
        });
        let exhausted = response
            .headers
            .get("x-esi-error-limit-remain")
            .is_some_and(|x| x == "0");
        let delay = if response.status == 429 {
            Some(delay.unwrap_or(60))
        } else if exhausted {
            Some(
                response
                    .headers
                    .get("x-esi-error-limit-reset")
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(60),
            )
        } else {
            delay
        };
        if let Some(delay) = delay {
            if let Ok(mut s) = self.state() {
                if s.generation == generation {
                    s.blocked_until = at.saturating_add(delay.min(86400));
                }
            }
        }
    }
    pub fn refresh_location(&self) -> Result<View, Error> {
        let _flight = self.flight()?;
        let (generation, access, character) = self.credentials()?;
        let etag = {
            let mut s = self.state()?;
            let session = s.session.as_mut().ok_or(Error::Disconnected)?;
            if let Some(l) = &mut session.location {
                if now() < l.fresh_until {
                    l.cached = true;
                    drop(s);
                    return self.status();
                }
            }
            session.etag.clone()
        };
        let result = (|| {
            let response = self.inner.http.send(Request {
                target: Target::Location { character, etag },
                bearer: Some(access),
            })?;
            self.limits(generation, &response);
            if !matches!(response.status, 200 | 304) {
                return Err(status_error(response.status));
            }
            let at = now();
            let ttl = cache_ttl(&response, at);
            let mut s = self.state()?;
            if s.generation != generation {
                return Err(Error::Cancelled);
            }
            let session = s.session.as_mut().ok_or(Error::Disconnected)?;
            if response.status == 304 {
                let l = session.location.as_mut().ok_or(Error::InvalidResponse)?;
                l.cached = true;
                l.fresh_until = at.saturating_add(ttl);
                l.stale = ttl == 0;
            } else {
                #[derive(Deserialize)]
                struct Wire {
                    solar_system_id: u64,
                }
                let l: Wire =
                    serde_json::from_slice(&response.body).map_err(|_| Error::InvalidResponse)?;
                valid_id(l.solar_system_id)?;
                let server_as_of = response
                    .headers
                    .get("last-modified")
                    .and_then(|x| httpdate::parse_http_date(x).ok())
                    .and_then(|d| d.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .filter(|t| *t <= at);
                session.location = Some(Location {
                    system_id: l.solar_system_id,
                    fetched_at: at,
                    server_as_of,
                    fresh_until: at.saturating_add(ttl),
                    cached: false,
                    stale: ttl == 0,
                });
            }
            session.etag = response.headers.get("etag").cloned().or_else(|| {
                if response.status == 304 {
                    session.etag.take()
                } else {
                    None
                }
            });
            s.error = None;
            drop(s);
            self.status()
        })();
        if let Err(e) = result {
            self.fail(generation, e);
        }
        result
    }
    pub fn prepare(&self, destination: u64, action: Action) -> Result<Confirmation, Error> {
        let _flight = self.flight()?;
        let (generation, _, _) = self.credentials()?;
        valid_id(destination)?;
        let mut s = self.state()?;
        if s.generation != generation {
            return Err(Error::Cancelled);
        }
        let character = s
            .session
            .as_ref()
            .ok_or(Error::Disconnected)?
            .character
            .clone();
        let confirmation = Confirmation {
            id: oauth::random()?.to_string(),
            character,
            destination,
            action,
            expires_at: now() + 30,
        };
        s.confirmation = Some(confirmation.clone());
        Ok(confirmation)
    }
    // The UI must first display this exact native-issued, one-use confirmation.
    // The ID is not a credential. No renderer-supplied character/body/flags accepted.
    pub fn confirmation_destination(&self, id: &str) -> Result<u64, Error> {
        let s = self.state()?;
        let c = s.confirmation.as_ref().ok_or(Error::InvalidConfirmation)?;
        if c.id != id || c.expires_at <= now() {
            return Err(Error::InvalidConfirmation);
        }
        Ok(c.destination)
    }
    pub fn send_waypoint(&self, confirmation_id: &str) -> Result<View, Error> {
        let _flight = self.flight()?;
        let (generation, access, character) = self.credentials()?;
        let confirmation = {
            let mut s = self.state()?;
            let c = s.confirmation.take().ok_or(Error::InvalidConfirmation)?;
            if c.id != confirmation_id
                || c.character.id != character
                || c.expires_at <= now()
                || s.generation != generation
            {
                return Err(Error::InvalidConfirmation);
            }
            c
        };
        let result = (|| {
            // Consumed before any network I/O: timeout/ambiguous outcome cannot replay.
            if !self.current(generation) {
                return Err(Error::Cancelled);
            }
            let response = self
                .inner
                .http
                .send(Request {
                    target: Target::Waypoint {
                        destination: confirmation.destination,
                        replace: confirmation.action == Action::SetDestination,
                    },
                    bearer: Some(access),
                })
                .map_err(|e| {
                    if matches!(
                        e,
                        Error::Timeout | Error::Unavailable | Error::InvalidResponse
                    ) {
                        Error::OutcomeUnknown
                    } else {
                        e
                    }
                })?;
            self.limits(generation, &response);
            if response.status != 204 {
                return Err(if response.status >= 500 {
                    Error::OutcomeUnknown
                } else {
                    status_error(response.status)
                });
            }
            if !self.current(generation) {
                // Disconnect cannot undo an already accepted request.
                return Err(Error::OutcomeUnknown);
            }
            self.status()
        })();
        if let Err(e) = result {
            self.fail(generation, e);
        }
        result
    }
}
fn valid_id(id: u64) -> Result<(), Error> {
    if id == 0 || id > MAX_ID {
        Err(Error::InvalidDestination)
    } else {
        Ok(())
    }
}
fn status_error(status: u16) -> Error {
    match status {
        401 => Error::Expired,
        403 => Error::Forbidden,
        429 => Error::RateLimited,
        _ => Error::Unavailable,
    }
}
fn cache_ttl(response: &Response, at: u64) -> u64 {
    let cache = response
        .headers
        .get("cache-control")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if cache
        .split(',')
        .any(|v| matches!(v.trim(), "no-store" | "no-cache"))
    {
        return 0;
    }
    let max_age = cache.split(',').find_map(|v| {
        v.trim()
            .strip_prefix("max-age=")
            .and_then(|v| v.trim_matches('"').parse::<u64>().ok())
    });
    let expiry = response
        .headers
        .get("expires")
        .and_then(|s| httpdate::parse_http_date(s).ok())
        .and_then(|d| d.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs().saturating_sub(at));
    let age = response
        .headers
        .get("age")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let apparent_age = response
        .headers
        .get("date")
        .and_then(|s| httpdate::parse_http_date(s).ok())
        .and_then(|d| d.duration_since(UNIX_EPOCH).ok())
        .map(|d| at.saturating_sub(d.as_secs()))
        .unwrap_or(0);
    max_age
        .map(|ttl| ttl.saturating_sub(age.max(apparent_age)))
        .unwrap_or(expiry.unwrap_or(0).saturating_sub(age))
        .min(3600)
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
}
impl Drop for TokenReply {
    fn drop(&mut self) {
        self.access_token.zeroize();
        if let Some(t) = &mut self.refresh_token {
            t.zeroize();
        }
    }
}
#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    code_challenge_methods_supported: Vec<String>,
}
fn validate_discovery(body: &[u8]) -> Result<(), Error> {
    let d: Discovery = serde_json::from_slice(body).map_err(|_| Error::Authentication)?;
    if d.issuer != "https://login.eveonline.com"
        || d.authorization_endpoint != AUTHORIZE
        || d.token_endpoint != TOKEN
        || d.jwks_uri != JWKS
        || !d
            .code_challenge_methods_supported
            .iter()
            .any(|v| v == "S256")
    {
        return Err(Error::Authentication);
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct Claims {
    iss: String,
    aud: Vec<String>,
    exp: u64,
    sub: String,
    name: String,
    scp: Vec<String>,
    nbf: Option<u64>,
    azp: Option<String>,
}
impl Claims {
    fn character_id(&self) -> Result<u64, Error> {
        let text = self
            .sub
            .strip_prefix("CHARACTER:EVE:")
            .ok_or(Error::Authentication)?;
        let v = text.parse::<u64>().map_err(|_| Error::Authentication)?;
        if text != v.to_string() {
            return Err(Error::Authentication);
        }
        valid_id(v).map_err(|_| Error::Authentication)?;
        Ok(v)
    }
}
fn verify(token: &str, body: &[u8], at: u64) -> Result<Claims, Error> {
    use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
    if token.len() > 16384 {
        return Err(Error::Authentication);
    }
    let header = decode_header(token).map_err(|_| Error::Authentication)?;
    if header.alg != Algorithm::RS256 {
        return Err(Error::Authentication);
    }
    let kid = header.kid.ok_or(Error::Authentication)?;
    if kid.len() > 128 {
        return Err(Error::Authentication);
    }
    #[derive(Deserialize)]
    struct Keys {
        keys: Vec<Key>,
    }
    #[derive(Deserialize)]
    struct Key {
        kid: Option<String>,
        kty: String,
        alg: Option<String>,
        #[serde(rename = "use")]
        usage: Option<String>,
        n: Option<String>,
        e: Option<String>,
    }
    let keys: Keys = serde_json::from_slice(body).map_err(|_| Error::Authentication)?;
    if keys.keys.len() > 32 {
        return Err(Error::Authentication);
    }
    let matching: Vec<_> = keys
        .keys
        .iter()
        .filter(|k| k.kid.as_ref() == Some(&kid))
        .collect();
    if matching.len() != 1 {
        return Err(Error::Authentication);
    }
    let key = matching[0];
    if key.kty != "RSA"
        || key.alg.as_deref() != Some("RS256")
        || key.usage.as_deref() != Some("sig")
    {
        return Err(Error::Authentication);
    }
    let decoding = DecodingKey::from_rsa_components(
        key.n.as_deref().ok_or(Error::Authentication)?,
        key.e.as_deref().ok_or(Error::Authentication)?,
    )
    .map_err(|_| Error::Authentication)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[
        "https://login.eveonline.com",
        "https://login.eveonline.com/",
        "login.eveonline.com",
    ]);
    validation.set_audience(&[CLIENT_ID, "EVE Online"]);
    validation.validate_exp = false; // Explicit strict injected-clock checks below (also in tests).
    let claims = decode::<Claims>(token, &decoding, &validation)
        .map_err(|_| Error::Authentication)?
        .claims;
    let required: Vec<_> = SCOPES.split(' ').collect();
    if claims.exp <= at
        || claims.exp.saturating_sub(at) > 86400
        || claims.nbf.is_some_and(|n| n > at)
        || !claims.aud.iter().any(|v| v == CLIENT_ID)
        || !claims.aud.iter().any(|v| v == "EVE Online")
        || claims.azp.as_deref().is_some_and(|v| v != CLIENT_ID)
        || claims.name.trim().is_empty()
        || claims.name.len() > 128
        || claims.scp.len() != 2
        || required
            .iter()
            .any(|scope| !claims.scp.iter().any(|v| v == scope))
    {
        return Err(Error::Authentication);
    }
    claims.character_id()?;
    Ok(claims)
}

/// Reuse fixed safe reader, then pin the exact approved snapshot BEFORE accepting
/// a renderer-selected system ID for an ESI write. No arbitrary station/structure IDs.
pub fn validate_destination(store: &super::portable::PortableStore, id: u64) -> Result<(), Error> {
    valid_id(id)?;
    let super::navcom::SnapshotRead::Text { text } = super::navcom::read(store) else {
        return Err(Error::InvalidDestination);
    };
    validate_snapshot(
        &text,
        "61c6e401dd97e52d4c236ed5dd16920a38dfa9c76fc637a17dc8a2e90ab9fc3b",
        id,
    )
}
fn validate_snapshot(text: &str, pin: &str, id: u64) -> Result<(), Error> {
    use sha2::{Digest, Sha256};
    let bytes = Sha256::digest(text.as_bytes());
    let actual = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    if actual != pin {
        return Err(Error::InvalidDestination);
    }
    let data: serde_json::Value =
        serde_json::from_str(text).map_err(|_| Error::InvalidDestination)?;
    if !data
        .get("systems")
        .and_then(|s| s.as_array())
        .is_some_and(|rows| {
            rows.iter()
                .any(|row| row.get(0).and_then(|v| v.as_u64()) == Some(id))
        })
    {
        return Err(Error::InvalidDestination);
    }
    Ok(())
}
