use super::{Error, Secret, CLIENT_ID, COMPATIBILITY_DATE, DISCOVERY, JWKS, TOKEN};
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use std::{collections::BTreeMap, io::Read, time::Duration};

pub(super) enum Target {
    Discovery,
    Jwks,
    Token {
        code: Secret,
        verifier: Secret,
    },
    Location {
        character: u64,
        etag: Option<String>,
    },
    Waypoint {
        destination: u64,
        replace: bool,
    },
}
pub(super) struct Request {
    pub target: Target,
    pub bearer: Option<Secret>,
}
pub(super) struct Response {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: zeroize::Zeroizing<Vec<u8>>,
}
pub(super) trait Transport: Send + Sync {
    fn send(&self, request: Request) -> Result<Response, Error>;
}
pub(super) struct CcpHttp;
impl Transport for CcpHttp {
    fn send(&self, request: Request) -> Result<Response, Error> {
        // No automatic retry (especially POST), redirect, cookie jar or disk cache.
        // Native root trust and inherited proxy settings remain enabled.
        let client = Client::builder()
            .https_only(true)
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .user_agent("ECHELON-OS/0.1 (https://github.com/ECHELON-APPLIED-SYSTEMS/echelon-os)")
            .build()
            .map_err(|_| Error::Unavailable)?;
        let mut response = client.execute(build(&client, request)?).map_err(|e| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Unavailable
            }
        })?;
        let mut headers = BTreeMap::new();
        for name in [
            "cache-control",
            "etag",
            "last-modified",
            "retry-after",
            "expires",
            "date",
            "age",
            "x-esi-error-limit-remain",
            "x-esi-error-limit-reset",
            "x-ratelimit-group",
            "x-ratelimit-limit",
            "x-ratelimit-remaining",
            "x-ratelimit-used",
        ] {
            if let Some(value) = response.headers().get(name).and_then(|v| v.to_str().ok()) {
                if value.len() <= 512 {
                    headers.insert(name.to_owned(), value.to_owned());
                }
            }
        }
        let status = response.status().as_u16();
        let length = response.content_length();
        let body = read_body(&mut response, length)?;
        Ok(Response {
            status,
            headers,
            body,
        })
    }
}

fn build(client: &Client, request: Request) -> Result<reqwest::blocking::Request, Error> {
    let mut builder = match request.target {
        Target::Discovery => client.get(DISCOVERY),
        Target::Jwks => client.get(JWKS),
        Target::Token { code, verifier } => client.post(TOKEN).form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("redirect_uri", super::REDIRECT_URI),
        ]),
        Target::Location { character, etag } => {
            let mut r = client.get(format!(
                "https://esi.evetech.net/characters/{character}/location"
            ));
            if let Some(etag) = etag {
                r = r.header("If-None-Match", etag);
            }
            r.header("X-Compatibility-Date", COMPATIBILITY_DATE)
        }
        Target::Waypoint {
            destination,
            replace,
        } => client
            .post("https://esi.evetech.net/ui/autopilot/waypoint")
            .header("X-Compatibility-Date", COMPATIBILITY_DATE)
            .query(&[
                ("destination_id", destination.to_string()),
                ("clear_other_waypoints", replace.to_string()),
                ("add_to_beginning", "false".to_owned()),
            ]),
    };
    if let Some(token) = request.bearer {
        builder = builder.bearer_auth(token.as_str());
    }
    builder.build().map_err(|_| Error::InvalidResponse)
}

fn read_body(reader: impl Read, length: Option<u64>) -> Result<zeroize::Zeroizing<Vec<u8>>, Error> {
    if length.is_some_and(|n| n > 131_072) {
        return Err(Error::InvalidResponse);
    }
    let mut body = zeroize::Zeroizing::new(Vec::new());
    reader
        .take(131_073)
        .read_to_end(&mut body)
        .map_err(|_| Error::Unavailable)?;
    if body.len() > 131_072 {
        return Err(Error::InvalidResponse);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_size_is_bounded_with_or_without_content_length() {
        let bytes = vec![0; 131_073];
        assert!(read_body(bytes.as_slice(), None).is_err());
        assert!(read_body(&bytes[..1], Some(131_073)).is_err());
        assert_eq!(read_body(&bytes[..131_072], None).unwrap().len(), 131_072);
    }
    #[test]
    fn actual_wire_requests_use_only_fixed_ccp_endpoints_and_exact_flags() {
        let client = Client::builder().build().unwrap();
        for replace in [true, false] {
            let r = build(
                &client,
                Request {
                    target: Target::Waypoint {
                        destination: 7,
                        replace,
                    },
                    bearer: Some(zeroize::Zeroizing::new("synthetic-bearer".into())),
                },
            )
            .unwrap();
            assert_eq!(r.method(), reqwest::Method::POST);
            assert_eq!(r.url().host_str(), Some("esi.evetech.net"));
            assert_eq!(r.url().path(), "/ui/autopilot/waypoint");
            let q: BTreeMap<_, _> = r.url().query_pairs().into_owned().collect();
            assert_eq!(q.len(), 3);
            assert_eq!(q["destination_id"], "7");
            assert_eq!(q["clear_other_waypoints"], replace.to_string());
            assert_eq!(q["add_to_beginning"], "false");
            assert_eq!(r.headers()["X-Compatibility-Date"], COMPATIBILITY_DATE);
            assert!(r.headers()["authorization"].is_sensitive());
        }
        let r = build(
            &client,
            Request {
                target: Target::Token {
                    code: zeroize::Zeroizing::new("synthetic-code".into()),
                    verifier: zeroize::Zeroizing::new("synthetic-verifier".into()),
                },
                bearer: None,
            },
        )
        .unwrap();
        assert_eq!(r.url().as_str(), TOKEN);
        let form: BTreeMap<_, _> = reqwest::Url::parse(&format!(
            "http://localhost/?{}",
            std::str::from_utf8(r.body().unwrap().as_bytes().unwrap()).unwrap()
        ))
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
        assert_eq!(form["redirect_uri"], super::super::REDIRECT_URI);
        assert_eq!(form["client_id"], CLIENT_ID);
        assert!(!form.contains_key("client_secret"));
        let r = build(
            &client,
            Request {
                target: Target::Location {
                    character: 42,
                    etag: Some("synthetic-etag".into()),
                },
                bearer: None,
            },
        )
        .unwrap();
        assert_eq!(
            r.url().as_str(),
            "https://esi.evetech.net/characters/42/location"
        );
        assert_eq!(r.headers()["If-None-Match"], "synthetic-etag");
    }
}
