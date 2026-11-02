use super::super::tests::{response, seeded, Call, Fake};
use super::*;
fn topology() -> Topology {
    Topology::parse(
        r#"{"edges":[[1,2],[2,3]],"systems":[[1,"Fiction A"],[2,null],[3,null]],"version":1}"#,
        &pin(),
        (3, 2, 0),
    )
    .unwrap()
}
fn pin() -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(
        br#"{"edges":[[1,2],[2,3]],"systems":[[1,"Fiction A"],[2,null],[3,null]],"version":1}"#,
    )
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}
fn ready(responses: Vec<Result<Response, Error>>) -> (Eve, Arc<Fake>) {
    let mock = Fake::new(responses);
    let eve = seeded(mock.clone());
    eve.state().unwrap().session.as_mut().unwrap().location = Some(Location {
        system_id: 1,
        fetched_at: now(),
        server_as_of: None,
        fresh_until: now() + 60,
        cached: false,
        stale: false,
    });
    (eve, mock)
}
fn accepted() -> Response {
    let mut r = response(204, serde_json::Value::Null);
    r.headers.insert("x-ratelimit-group".into(), "ui".into());
    r.headers
        .insert("x-ratelimit-remaining".into(), "100".into());
    r
}
#[test]
fn serial_flags_order_counts_no_replay_or_prepare_writes() {
    let (eve, mock) = ready(vec![Ok(accepted()), Ok(accepted())]);
    let c = eve
        .prepare_route_with(&[1, 2, 3], || Ok(topology()))
        .unwrap();
    assert!(mock.calls.lock().unwrap().is_empty());
    let r = eve
        .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
        .unwrap();
    assert_eq!(r.delivery, Delivery::AllAccepted);
    assert_eq!(r.accepted_count, 2);
    assert_eq!(r.last_accepted, Some(3));
    assert_eq!(r.next_unsent, None);
    assert_eq!(
        *mock.calls.lock().unwrap(),
        vec![Call::Waypoint(2, true), Call::Waypoint(3, false)]
    );
    assert!(eve
        .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
        .is_err());
}
#[test]
fn invalid_paths_caps_schema_and_pin_are_refused() {
    for ids in [
        vec![],
        vec![1],
        vec![1, 3],
        vec![1, 2, 1],
        vec![0, 2],
        vec![1, 99],
        (1..13).collect(),
    ] {
        let (eve, mock) = ready(vec![]);
        assert!(eve.prepare_route_with(&ids, || Ok(topology())).is_err());
        assert!(mock.calls.lock().unwrap().is_empty());
    }
    assert!(Topology::parse("{}", "wrong", (0, 0, 0)).is_err());
    assert!(serde_json::from_str::<RouteInput>(
        &serde_json::to_string(&(1..13).collect::<Vec<_>>()).unwrap()
    )
    .is_err());
    let t = topology();
    assert!(t.validate(&[1, 2]).is_ok());
}
#[test]
fn fresh_origin_character_expiry_cancel_and_changed_snapshot_guards() {
    let (eve, mock) = ready(vec![]);
    eve.state()
        .unwrap()
        .session
        .as_mut()
        .unwrap()
        .location
        .as_mut()
        .unwrap()
        .system_id = 2;
    assert!(matches!(
        eve.prepare_route_with(&[1, 2], || Ok(topology())),
        Err(Error::OriginMismatch)
    ));
    eve.state()
        .unwrap()
        .session
        .as_mut()
        .unwrap()
        .location
        .as_mut()
        .unwrap()
        .system_id = 1;
    let c = eve.prepare_route_with(&[1, 2], || Ok(topology())).unwrap();
    eve.cancel_route(&c.id).unwrap();
    assert!(eve
        .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
        .is_err());
    let c = eve.prepare_route_with(&[1, 2], || Ok(topology())).unwrap();
    assert_eq!(
        eve.send_route_with(&c.id, || Err(Error::RouteInvalid), |_| {}, WORK)
            .unwrap()
            .reason,
        Some(Error::RouteInvalid)
    );
    let c = eve.prepare_route_with(&[1, 2], || Ok(topology())).unwrap();
    eve.state().unwrap().session.as_mut().unwrap().character.id = 43;
    assert_eq!(
        eve.send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
            .unwrap()
            .accepted_count,
        0
    );
    eve.state().unwrap().session.as_mut().unwrap().character.id = 42;
    let c = eve.prepare_route_with(&[1, 2], || Ok(topology())).unwrap();
    eve.state()
        .unwrap()
        .route_confirmation
        .as_mut()
        .unwrap()
        .confirmation
        .expires_at = now();
    assert!(eve
        .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
        .is_err());
    eve.state()
        .unwrap()
        .session
        .as_mut()
        .unwrap()
        .location
        .as_mut()
        .unwrap()
        .fresh_until = now();
    assert!(matches!(
        eve.prepare_route_with(&[1, 2], || Ok(topology())),
        Err(Error::OriginStale)
    ));
    assert!(mock.calls.lock().unwrap().is_empty());
}
#[test]
fn partial_unknown_rejected_rate_and_missing_headroom_never_retry() {
    for second in [
        Err(Error::Timeout),
        Ok(response(503, serde_json::Value::Null)),
        Ok(response(429, serde_json::Value::Null)),
    ] {
        let (eve, mock) = ready(vec![Ok(accepted()), second]);
        let c = eve
            .prepare_route_with(&[1, 2, 3], || Ok(topology()))
            .unwrap();
        let r = eve
            .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
            .unwrap();
        assert_eq!(r.accepted_count, 1);
        assert_eq!(r.last_accepted, Some(2));
        assert_eq!(r.failed_system, Some(3));
        assert_eq!(mock.calls.lock().unwrap().len(), 2);
        assert!(eve
            .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
            .is_err());
    }
    for first in [response(204, serde_json::Value::Null), {
        let mut r = accepted();
        r.headers.insert("x-ratelimit-remaining".into(), "2".into());
        r
    }] {
        let (eve, mock) = ready(vec![Ok(first)]);
        let c = eve
            .prepare_route_with(&[1, 2, 3], || Ok(topology()))
            .unwrap();
        let r = eve
            .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
            .unwrap();
        assert_eq!(r.accepted_count, 1);
        assert_eq!(r.reason, Some(Error::RouteBudget));
        assert_eq!(r.next_unsent, Some(3));
        assert_eq!(mock.calls.lock().unwrap().len(), 1);
    }
}
#[test]
fn cancellation_snapshot_change_revision_and_total_budget_stop_later_requests() {
    let (eve, mock) = ready(vec![Ok(accepted())]);
    let c = eve
        .prepare_route_with(&[1, 2, 3], || Ok(topology()))
        .unwrap();
    let cancel = eve.clone();
    let r = eve
        .send_route_with(
            &c.id,
            || Ok(topology()),
            move |_| {
                cancel.disconnect().unwrap();
            },
            WORK,
        )
        .unwrap();
    assert_eq!(r.accepted_count, 1);
    assert_eq!(mock.calls.lock().unwrap().len(), 1);
    let (eve, mock) = ready(vec![Ok(accepted())]);
    let c = eve
        .prepare_route_with(&[1, 2, 3], || Ok(topology()))
        .unwrap();
    let mut loads = 0;
    let r = eve
        .send_route_with(
            &c.id,
            || {
                loads += 1;
                if loads == 2 {
                    Err(Error::RouteInvalid)
                } else {
                    Ok(topology())
                }
            },
            |_| {},
            WORK,
        )
        .unwrap();
    assert_eq!(r.accepted_count, 1);
    assert_eq!(r.reason, Some(Error::RouteInvalid));
    assert_eq!(mock.calls.lock().unwrap().len(), 1);
    let (eve, mock) = ready(vec![]);
    let c = eve.prepare_route_with(&[1, 2], || Ok(topology())).unwrap();
    let r = eve
        .send_route_with(&c.id, || Ok(topology()), |_| {}, Duration::ZERO)
        .unwrap();
    assert_eq!(r.reason, Some(Error::RouteTime));
    assert!(mock.calls.lock().unwrap().is_empty());
    let c = eve.prepare_route_with(&[1, 2], || Ok(topology())).unwrap();
    eve.state().unwrap().location_revision += 1;
    let r = eve
        .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
        .unwrap();
    assert_eq!(r.accepted_count, 0);
}

#[test]
fn cap_ten_posts_pacing_single_hop_and_rate_headers() {
    let text=serde_json::json!({"version":1,"systems":(1..=11).map(|i|serde_json::json!([i,null])).collect::<Vec<_>>(),"edges":(1..11).map(|i|[i,i+1]).collect::<Vec<_>>()}).to_string();
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let load = || Topology::parse(&text, &hash, (11, 10, 0));
    let (eve, mock) = ready((0..10).map(|_| Ok(accepted())).collect());
    let systems: Vec<_> = (1..=11).collect();
    let c = eve.prepare_route_with(&systems, load).unwrap();
    let mut paused = Duration::ZERO;
    let r = eve
        .send_route_with(&c.id, load, |d| paused += d, WORK)
        .unwrap();
    assert_eq!(r.accepted_count, 10);
    assert_eq!(paused, Duration::from_secs(27));
    assert_eq!(mock.calls.lock().unwrap().len(), 10);
    let (eve, mock) = ready(vec![Ok(response(204, serde_json::Value::Null))]);
    let c = eve.prepare_route_with(&[1, 2], || Ok(topology())).unwrap();
    let r = eve
        .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
        .unwrap();
    assert_eq!(r.delivery, Delivery::AllAccepted);
    assert_eq!(*mock.calls.lock().unwrap(), vec![Call::Waypoint(2, true)]);
    for header in ["retry-after", "x-esi-error-limit-remain"] {
        let mut response = accepted();
        response.headers.insert(
            header.into(),
            if header == "retry-after" { "10" } else { "0" }.into(),
        );
        let (eve, mock) = ready(vec![Ok(response)]);
        let c = eve
            .prepare_route_with(&[1, 2, 3], || Ok(topology()))
            .unwrap();
        let r = eve
            .send_route_with(&c.id, || Ok(topology()), |_| {}, WORK)
            .unwrap();
        assert_eq!(r.accepted_count, 1);
        assert_eq!(r.reason, Some(Error::RateLimited));
        assert_eq!(mock.calls.lock().unwrap().len(), 1);
    }
}
#[test]
fn active_cancellation_no_single_flight_interleave_and_preflight_budget() {
    let (eve, mock) = ready(vec![Ok(accepted())]);
    let c = eve
        .prepare_route_with(&[1, 2, 3], || Ok(topology()))
        .unwrap();
    let cancel = eve.clone();
    let id = c.id.clone();
    let r = eve
        .send_route_with(
            &c.id,
            || Ok(topology()),
            move |_| {
                let _ = cancel.cancel_route(&id);
                assert!(matches!(
                    cancel.prepare_route_with(&[1, 2], || Ok(topology())),
                    Err(Error::Busy)
                ));
            },
            WORK,
        )
        .unwrap();
    assert_eq!(r.accepted_count, 1);
    assert_eq!(r.reason, Some(Error::Cancelled));
    assert_eq!(mock.calls.lock().unwrap().len(), 1);
    eve.state().unwrap().ui_remaining = Some(1);
    assert!(matches!(
        eve.prepare_route_with(&[1, 2], || Ok(topology())),
        Err(Error::RouteBudget)
    ));
}
