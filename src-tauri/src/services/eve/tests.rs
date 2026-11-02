//! Synthetic local test key/JWKS only: not a CCP key, account credential or usable
//! production secret. JWTs are generated in memory; never saved or printed.
use super::*;
use std::{
    collections::{BTreeMap, VecDeque},
    io::Write,
    net::TcpStream,
};
static LOOPBACK_TEST: Mutex<()> = Mutex::new(());
fn claims() -> Claims {
    Claims {
        iss: "https://login.eveonline.com".into(),
        aud: vec![CLIENT_ID.into(), "EVE Online".into()],
        exp: now() + 600,
        sub: "CHARACTER:EVE:42".into(),
        name: "Fiction Pilot".into(),
        scp: SCOPES.split(' ').map(str::to_owned).collect(),
        nbf: None,
        azp: Some(CLIENT_ID.into()),
    }
}
fn sign(c: &Claims) -> String {
    let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    h.kid = Some("synthetic-unit-test".into());
    jsonwebtoken::encode(
        &h,
        c,
        &jsonwebtoken::EncodingKey::from_rsa_der(include_bytes!("synthetic-test-key.der")),
    )
    .unwrap()
}
fn keys() -> &'static [u8] {
    include_bytes!("synthetic-test-jwks.json")
}
fn response(status: u16, body: serde_json::Value) -> Response {
    Response {
        status,
        headers: BTreeMap::new(),
        body: Zeroizing::new(serde_json::to_vec(&body).unwrap()),
    }
}
fn discovery() -> Response {
    response(
        200,
        serde_json::json!({"issuer":"https://login.eveonline.com","authorization_endpoint":AUTHORIZE,"token_endpoint":TOKEN,"jwks_uri":JWKS,"code_challenge_methods_supported":["S256"]}),
    )
}
#[derive(Debug, PartialEq, Eq)]
enum Call {
    Discovery,
    Jwks,
    Token,
    Location(u64),
    Waypoint(u64, bool),
}
struct Fake {
    responses: Mutex<VecDeque<Result<Response, Error>>>,
    calls: Mutex<Vec<Call>>,
}
impl Fake {
    fn new(responses: Vec<Result<Response, Error>>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            calls: Mutex::new(vec![]),
        })
    }
}
impl Transport for Fake {
    fn send(&self, r: Request) -> Result<Response, Error> {
        let call = match r.target {
            Target::Discovery => Call::Discovery,
            Target::Jwks => Call::Jwks,
            Target::Token { code, verifier } => {
                assert!(!code.is_empty());
                assert_eq!(verifier.len(), 43);
                assert!(r.bearer.is_none());
                Call::Token
            }
            Target::Location { character, .. } => {
                assert!(r.bearer.is_some());
                Call::Location(character)
            }
            Target::Waypoint {
                destination,
                replace,
            } => {
                assert!(r.bearer.is_some());
                Call::Waypoint(destination, replace)
            }
        };
        self.calls.lock().unwrap().push(call);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("mock response required")
    }
}
fn seeded(fake: Arc<Fake>) -> Eve {
    let eve = Eve::new(fake);
    {
        let mut s = eve.state().unwrap();
        s.phase = Phase::Connected;
        s.session = Some(Session {
            access: Zeroizing::new("synthetic-test-bearer".into()),
            _expiry_cancel: None,
            character: Character {
                id: 42,
                name: "Fiction Pilot".into(),
            },
            expires: now() + 600,
            location: None,
            etag: None,
        });
    }
    eve
}
fn request(query: &str, host: &str) -> Vec<u8> {
    format!("GET /oauth/callback?{query} HTTP/1.1\r\nHost: {host}\r\n\r\n").into_bytes()
}
#[test]
fn pkce_and_exact_callback_security() {
    assert_eq!(
        oauth::challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    let verifier = oauth::random().unwrap();
    let state = oauth::random().unwrap();
    assert_eq!(state.len(), 43);
    assert_ne!(state.as_str(), verifier.as_str());
    let u = oauth::authorize_url(&state, &verifier).unwrap();
    let q: BTreeMap<_, _> = u.query_pairs().into_owned().collect();
    assert_eq!(q["redirect_uri"], REDIRECT_URI);
    assert_eq!(q["scope"], SCOPES);
    assert_eq!(q["code_challenge_method"], "S256");
    assert!(!q.contains_key("client_secret"));
    assert!(oauth::callback(
        &request("code=fiction&state=expected", "localhost:43173"),
        "expected"
    )
    .is_ok());
    for raw in [
        request("code=fiction&state=wrong", "localhost:43173"),
        request("code=a&code=b&state=expected", "localhost:43173"),
        request("code=a&state=expected", "127.0.0.1:43173"),
        request("code=a&state=expected", "localhost:9"),
        request(
            "error=access_denied&code=a&state=expected",
            "localhost:43173",
        ),
        request("code=a&state=expected&unknown=x", "localhost:43173"),
        b"GET /other HTTP/1.1\r\nHost: localhost:43173\r\n\r\n".to_vec(),
        vec![b'x'; 8193],
    ] {
        assert!(oauth::callback(&raw, "expected").is_err());
    }
    assert!(matches!(
        oauth::callback(
            &request("error=access_denied&state=expected", "localhost:43173"),
            "expected"
        ),
        Err(Error::Denied)
    ));
    assert!(oauth::browser(&reqwest::Url::parse("https://example.com/").unwrap()).is_err());
}
#[test]
fn signed_jwt_and_adversarial_claims() {
    let good = claims();
    let token = sign(&good);
    assert!(verify(&token, keys(), now()).is_ok());
    let bad_claims: Vec<Claims> = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
        .into_iter()
        .map(|case| {
            let mut c = claims();
            match case {
                0 => c.iss = "https://example.com".into(),
                1 => c.aud = vec!["EVE Online".into()],
                2 => c.exp = now(),
                3 => c.sub = "CHARACTER:EVE:0".into(),
                4 => c.scp = vec!["unapproved.scope".into()],
                5 => c.azp = Some("wrong-client".into()),
                6 => c.nbf = Some(now() + 600),
                7 => c.sub = "CHARACTER:EVE:+42".into(),
                8 => c.sub = "CHARACTER:EVE:042".into(),
                9 => c.exp = now() + 86401,
                10 => c.name = " ".into(),
                _ => c.scp.push("unapproved.scope".into()),
            }
            c
        })
        .collect();
    for c in bad_claims {
        assert!(verify(&sign(&c), keys(), now()).is_err());
    }
    let mut tampered = token.into_bytes();
    let pos = tampered.iter().rposition(|b| *b == b'.').unwrap() + 2;
    tampered[pos] = if tampered[pos] == b'A' { b'B' } else { b'A' };
    assert!(verify(std::str::from_utf8(&tampered).unwrap(), keys(), now()).is_err());
    assert!(verify(&sign(&good), br#"{"keys":[]}"#, now()).is_err());
    let mut hmac_header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    hmac_header.kid = Some("synthetic-unit-test".into());
    let hmac = jsonwebtoken::encode(
        &hmac_header,
        &good,
        &jsonwebtoken::EncodingKey::from_secret(keys()),
    )
    .unwrap();
    assert!(verify(&hmac, keys(), now()).is_err());
    let mut jwk: serde_json::Value = serde_json::from_slice(keys()).unwrap();
    jwk["keys"][0]["alg"] = "HS256".into();
    assert!(verify(&sign(&good), &serde_json::to_vec(&jwk).unwrap(), now()).is_err());
    assert!(verify(&"x".repeat(16385), keys(), now()).is_err());
}
#[test]
fn discovery_never_routes_to_untrusted_host_or_path() {
    assert!(validate_discovery(&discovery().body).is_ok());
    for field in [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "jwks_uri",
    ] {
        let mut v: serde_json::Value = serde_json::from_slice(&discovery().body).unwrap();
        v[field] = "https://example.com/".into();
        assert!(validate_discovery(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
#[test]
fn local_connect_port_conflict_disconnect_and_no_token_ipc() {
    let _serial = LOOPBACK_TEST.lock().unwrap();
    let token = sign(&claims());
    let fake = Fake::new(vec![
        Ok(discovery()),
        Ok(response(
            200,
            serde_json::json!({"access_token":token,"refresh_token":"synthetic-unused-refresh","token_type":"Bearer","expires_in":600}),
        )),
        Ok(Response {
            status: 200,
            headers: BTreeMap::new(),
            body: Zeroizing::new(keys().to_vec()),
        }),
    ]);
    let eve = Eve::new(fake.clone());
    let occupied = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 43173)).unwrap();
    assert!(matches!(
        eve.connect_with(|_| Ok(())),
        Err(Error::PortOccupied)
    ));
    drop(occupied);
    let result = eve
        .connect_with(|url| {
            let q: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
            let mut bad = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, 43173)).unwrap();
            bad.write_all(&request("code=fiction&state=wrong", "localhost:43173"))
                .unwrap();
            let mut stream = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, 43173)).unwrap();
            stream
                .write_all(&request(
                    &format!("code=fiction&state={}", q["state"]),
                    "localhost:43173",
                ))
                .unwrap();
            Ok(())
        })
        .unwrap();
    assert_eq!(result.phase, Phase::Connected);
    assert_eq!(result.character.unwrap().id, 42);
    let safe = serde_json::to_string(&eve.status().unwrap()).unwrap();
    assert!(!safe.contains("synthetic-unused-refresh"));
    assert!(!safe.contains("access_token"));
    assert!(!safe.contains(&token));
    assert!(TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, 43173)).is_err());
    eve.disconnect().unwrap();
    assert!(eve.state().unwrap().session.is_none());
    assert!(eve.state().unwrap().confirmation.is_none());
    assert_eq!(
        *fake.calls.lock().unwrap(),
        vec![Call::Discovery, Call::Token, Call::Jwks]
    );
}
#[test]
fn callback_timeout_cancel_and_disconnect_during_connect() {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    assert!(matches!(
        oauth::receive(listener, "s", || false, Duration::from_millis(5)),
        Err(Error::Timeout)
    ));
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    assert!(matches!(
        oauth::receive(listener, "s", || true, Duration::from_secs(1)),
        Err(Error::Cancelled)
    ));
    let _serial = LOOPBACK_TEST.lock().unwrap();
    let fake = Fake::new(vec![Ok(discovery())]);
    let eve = Eve::new(fake);
    let cancel = eve.clone();
    assert!(matches!(
        eve.connect_with(move |_| {
            cancel.disconnect()?;
            Ok(())
        }),
        Err(Error::Cancelled)
    ));
    assert_eq!(eve.status().unwrap().phase, Phase::Disconnected);
}
#[test]
fn cached_location_rate_limit_and_expiry() {
    let mut initial = response(200, serde_json::json!({"solar_system_id":7}));
    initial
        .headers
        .insert("cache-control".into(), "max-age=5".into());
    initial.headers.insert("etag".into(), "fiction-tag".into());
    let fake = Fake::new(vec![Ok(initial)]);
    let eve = seeded(fake.clone());
    assert!(!eve.refresh_location().unwrap().location.unwrap().cached);
    assert!(eve.refresh_location().unwrap().location.unwrap().cached);
    assert_eq!(fake.calls.lock().unwrap().len(), 1);
    let mut limited = response(429, serde_json::json!({}));
    limited.headers.insert("retry-after".into(), "30".into());
    let fake = Fake::new(vec![Ok(limited)]);
    let eve = seeded(fake.clone());
    assert!(matches!(eve.refresh_location(), Err(Error::RateLimited)));
    assert!(matches!(eve.refresh_location(), Err(Error::RateLimited)));
    assert_eq!(fake.calls.lock().unwrap().len(), 1);
    assert!(eve.status().unwrap().retry_at.is_some());
    eve.state().unwrap().session.as_mut().unwrap().expires = now();
    assert_eq!(eve.status().unwrap().phase, Phase::Expired);
    assert!(eve.state().unwrap().session.is_none());
}
#[test]
fn confirmed_one_shot_writes_flags_replay_and_ambiguous_timeout() {
    let fake = Fake::new(vec![
        Ok(response(204, serde_json::json!(null))),
        Ok(response(204, serde_json::json!(null))),
        Err(Error::Timeout),
    ]);
    let eve = seeded(fake.clone());
    assert!(matches!(
        eve.send_waypoint("unknown"),
        Err(Error::InvalidConfirmation)
    ));
    assert!(fake.calls.lock().unwrap().is_empty());
    for action in [Action::SetDestination, Action::AddWaypoint] {
        let c = eve.prepare(7, action).unwrap();
        assert_eq!(c.character.id, 42);
        let before = fake.calls.lock().unwrap().len();
        assert_eq!(eve.confirmation_destination(&c.id).unwrap(), 7);
        assert_eq!(fake.calls.lock().unwrap().len(), before);
        eve.send_waypoint(&c.id).unwrap();
        assert!(matches!(
            eve.send_waypoint(&c.id),
            Err(Error::InvalidConfirmation)
        ));
    }
    let c = eve.prepare(7, Action::SetDestination).unwrap();
    assert!(matches!(
        eve.send_waypoint(&c.id),
        Err(Error::OutcomeUnknown)
    ));
    assert!(matches!(
        eve.send_waypoint(&c.id),
        Err(Error::InvalidConfirmation)
    ));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        vec![
            Call::Waypoint(7, true),
            Call::Waypoint(7, false),
            Call::Waypoint(7, true)
        ]
    );
}
#[test]
fn confirmation_character_expiry_busy_and_logout_guards() {
    let fake = Fake::new(vec![]);
    let eve = seeded(fake.clone());
    assert!(matches!(
        eve.prepare(0, Action::SetDestination),
        Err(Error::InvalidDestination)
    ));
    let c = eve.prepare(7, Action::AddWaypoint).unwrap();
    eve.state()
        .unwrap()
        .confirmation
        .as_mut()
        .unwrap()
        .character
        .id = 99;
    assert!(matches!(
        eve.send_waypoint(&c.id),
        Err(Error::InvalidConfirmation)
    ));
    let c = eve.prepare(7, Action::AddWaypoint).unwrap();
    eve.state()
        .unwrap()
        .confirmation
        .as_mut()
        .unwrap()
        .expires_at = now();
    assert!(matches!(
        eve.send_waypoint(&c.id),
        Err(Error::InvalidConfirmation)
    ));
    let guard = eve.flight().unwrap();
    assert!(matches!(eve.refresh_location(), Err(Error::Busy)));
    drop(guard);
    eve.prepare(7, Action::AddWaypoint).unwrap();
    eve.disconnect().unwrap();
    assert!(eve.state().unwrap().confirmation.is_none());
    assert!(matches!(
        eve.send_waypoint("anything"),
        Err(Error::Disconnected)
    ));
    assert!(fake.calls.lock().unwrap().is_empty());
}
#[test]
fn cache_no_store_and_malformed_location() {
    let mut r = response(200, serde_json::json!({}));
    r.headers
        .insert("cache-control".into(), "no-store, max-age=999".into());
    assert_eq!(cache_ttl(&r, now()), 0);
    let fake = Fake::new(vec![Ok(response(
        200,
        serde_json::json!({"solar_system_id":0}),
    ))]);
    let eve = seeded(fake);
    assert!(eve.refresh_location().is_err());
    assert!(eve.status().unwrap().location.is_none());
}

#[test]
fn destination_requires_exact_pin_and_membership_before_writes() {
    use sha2::{Digest, Sha256};
    let text = r#"{"version":1,"systems":[[7,"Synthetic Seven"]],"edges":[]}"#;
    let pin = Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert!(validate_snapshot(text, &pin, 7).is_ok());
    assert_eq!(
        validate_snapshot(text, &pin, 8),
        Err(Error::InvalidDestination)
    );
    assert_eq!(
        validate_snapshot(text, &"0".repeat(64), 7),
        Err(Error::InvalidDestination)
    );
    assert_eq!(
        validate_snapshot(&format!("{text} "), &pin, 7),
        Err(Error::InvalidDestination)
    );
    let root = std::env::temp_dir().join(format!("echelon-eve-pin-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    {
        let store =
            super::super::portable::PortableStore::open(&root.join("echelon-os.exe")).unwrap();
        assert_eq!(
            validate_destination(&store, 7),
            Err(Error::InvalidDestination)
        );
        std::fs::create_dir(store.root().join("navcom")).unwrap();
        std::fs::write(store.root().join("navcom/snapshot.json"), text).unwrap();
        assert_eq!(
            validate_destination(&store, 7),
            Err(Error::InvalidDestination)
        );
        assert_eq!(
            validate_destination(&store, 0),
            Err(Error::InvalidDestination)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn revocation_not_modified_and_cache_age_are_honest() {
    let mut r = response(200, serde_json::json!({"solar_system_id":7}));
    r.headers.insert("cache-control".into(), "max-age=5".into());
    r.headers.insert(
        "date".into(),
        httpdate::fmt_http_date(UNIX_EPOCH + Duration::from_secs(now() - 10)),
    );
    assert_eq!(cache_ttl(&r, now()), 0);
    for status in [401, 403] {
        let mock = Fake::new(vec![Ok(response(status, serde_json::Value::Null))]);
        let eve = seeded(mock);
        assert!(eve.refresh_location().is_err());
        assert!(eve.state().unwrap().session.is_none());
        assert!(eve.status().unwrap().character.is_none());
    }
    let mut first = response(200, serde_json::json!({"solar_system_id":7}));
    first
        .headers
        .insert("etag".into(), "synthetic-version".into());
    let mut second = response(304, serde_json::Value::Null);
    second
        .headers
        .insert("cache-control".into(), "max-age=5".into());
    let mock = Fake::new(vec![Ok(first), Ok(second)]);
    let eve = seeded(mock.clone());
    let before = eve.refresh_location().unwrap().location.unwrap();
    let after = eve.refresh_location().unwrap().location.unwrap();
    assert_eq!(before.fetched_at, after.fetched_at);
    assert!(after.cached && !after.stale);
    assert_eq!(mock.calls.lock().unwrap().len(), 2);
}
