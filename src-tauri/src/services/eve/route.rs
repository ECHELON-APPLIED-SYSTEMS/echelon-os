//! Explicit bounded waypoint transfer; no movement, polling or automatic retry.
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    time::Instant,
};
pub const MAX_POSTS: usize = 10;
const PACE: Duration = Duration::from_secs(3);
const WORK: Duration = Duration::from_secs(120);
const PIN: &str = "61c6e401dd97e52d4c236ed5dd16920a38dfa9c76fc637a17dc8a2e90ab9fc3b";
#[derive(Debug, Clone, Serialize)]
pub struct RouteConfirmation {
    pub id: String,
    pub character: Character,
    pub systems: Vec<u64>,
    pub expires_at: u64,
}
pub(super) struct Pending {
    pub confirmation: RouteConfirmation,
    pub generation: u64,
    pub revision: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Delivery {
    AllAccepted,
    Stopped,
    Unknown,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub delivery: Delivery,
    pub total: usize,
    pub accepted_count: usize,
    pub last_accepted: Option<u64>,
    pub failed_system: Option<u64>,
    pub next_unsent: Option<u64>,
    pub reason: Option<Error>,
}
/// Bounded IPC sequence deserializer: rejects oversized arrays before allocation.
pub struct RouteInput(pub Vec<u64>);
impl<'de> Deserialize<'de> for RouteInput {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = RouteInput;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("2 to 11 system identifiers")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                while let Some(id) = seq.next_element::<u64>()? {
                    if v.len() > MAX_POSTS {
                        return Err(serde::de::Error::custom("route operation cap exceeded"));
                    }
                    v.push(id);
                }
                Ok(RouteInput(v))
            }
        }
        d.deserialize_seq(V)
    }
}
struct Topology {
    adj: BTreeMap<u64, Vec<u64>>,
}
impl Topology {
    fn load(root: &Path) -> Result<Self, Error> {
        let text = super::super::navcom::read_root(root)
            .map_err(|_| Error::RouteInvalid)?
            .ok_or(Error::RouteInvalid)?;
        Self::parse(&text, PIN, (8490, 6989, 3222))
    }
    fn parse(text: &str, pin: &str, counts: (usize, usize, usize)) -> Result<Self, Error> {
        use sha2::{Digest, Sha256};
        let actual = Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if text.len() > 4 * 1024 * 1024 || actual != pin {
            return Err(Error::RouteInvalid);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Snapshot {
            version: u8,
            systems: Vec<(u64, Option<String>)>,
            edges: Vec<(u64, u64)>,
        }
        let d: Snapshot = serde_json::from_str(text).map_err(|_| Error::RouteInvalid)?;
        if d.version != 1
            || d.systems.len() != counts.0
            || d.edges.len() != counts.1
            || d.systems.len() > 10000
            || d.edges.len() > 50000
        {
            return Err(Error::RouteInvalid);
        }
        let mut adj = BTreeMap::new();
        let mut previous = 0;
        for (id, name) in d.systems {
            if valid_id(id).is_err()
                || id <= previous
                || name.is_some_and(|s| s.is_empty() || s.chars().count() > 256)
            {
                return Err(Error::RouteInvalid);
            }
            adj.insert(id, Vec::new());
            previous = id;
        }
        let mut prior = (0, 0);
        for (a, b) in d.edges {
            if a >= b || (a, b) <= prior || !adj.contains_key(&a) || !adj.contains_key(&b) {
                return Err(Error::RouteInvalid);
            }
            adj.get_mut(&a).ok_or(Error::RouteInvalid)?.push(b);
            adj.get_mut(&b).ok_or(Error::RouteInvalid)?.push(a);
            prior = (a, b);
        }
        if adj.values().filter(|n| n.is_empty()).count() != counts.2 {
            return Err(Error::RouteInvalid);
        }
        for n in adj.values_mut() {
            n.sort_unstable();
        }
        Ok(Self { adj })
    }
    fn validate(&self, systems: &[u64]) -> Result<(), Error> {
        bounds(systems)?;
        let unique: BTreeSet<_> = systems.iter().copied().collect();
        if unique.len() != systems.len()
            || systems.iter().any(|id| !self.adj.contains_key(id))
            || systems.windows(2).any(|p| !self.adj[&p[0]].contains(&p[1]))
        {
            return Err(Error::RouteInvalid);
        }
        // Recompute the deterministic minimum-hop path (ascending-neighbor BFS),
        // matching the current manual planner; reject arbitrary detours/lists.
        let origin = systems[0];
        let destination = *systems.last().ok_or(Error::RouteInvalid)?;
        let mut parent = BTreeMap::from([(origin, None)]);
        let mut queue = VecDeque::from([origin]);
        while let Some(id) = queue.pop_front() {
            if id == destination {
                break;
            }
            for &next in &self.adj[&id] {
                if let std::collections::btree_map::Entry::Vacant(e) = parent.entry(next) {
                    e.insert(Some(id));
                    queue.push_back(next);
                }
            }
        }
        if !parent.contains_key(&destination) {
            return Err(Error::RouteInvalid);
        }
        let mut path = vec![destination];
        let mut id = destination;
        while let Some(Some(next)) = parent.get(&id) {
            id = *next;
            path.push(id);
        }
        path.reverse();
        if path != systems {
            return Err(Error::RouteInvalid);
        }
        Ok(())
    }
}
fn bounds(systems: &[u64]) -> Result<(), Error> {
    if systems.len() > MAX_POSTS + 1 {
        return Err(Error::RouteTooLong);
    }
    if systems.len() < 2 || systems.iter().any(|id| valid_id(*id).is_err()) {
        return Err(Error::RouteInvalid);
    }
    Ok(())
}
impl Eve {
    fn origin(
        &self,
        generation: u64,
        character: u64,
        origin: u64,
        revision: Option<u64>,
    ) -> Result<u64, Error> {
        let s = self.state()?;
        let session = s.session.as_ref().ok_or(Error::Disconnected)?;
        if s.generation != generation
            || session.character.id != character
            || revision.is_some_and(|v| v != s.location_revision)
        {
            return Err(Error::InvalidConfirmation);
        }
        let location = session.location.as_ref().ok_or(Error::OriginStale)?;
        if location.system_id != origin {
            return Err(Error::OriginMismatch);
        }
        if location.fresh_until <= now() {
            return Err(Error::OriginStale);
        }
        Ok(location.fresh_until)
    }
    pub fn prepare_route(&self, root: &Path, systems: &[u64]) -> Result<RouteConfirmation, Error> {
        self.prepare_route_with(systems, || Topology::load(root))
    }
    fn prepare_route_with(
        &self,
        systems: &[u64],
        load: impl FnOnce() -> Result<Topology, Error>,
    ) -> Result<RouteConfirmation, Error> {
        let _flight = self.flight()?;
        bounds(systems)?;
        let (generation, _, character) = self.credentials()?;
        load()?.validate(systems)?;
        let fresh = self.origin(generation, character, systems[0], None)?;
        let mut s = self.state()?;
        if s.generation != generation {
            return Err(Error::Cancelled);
        }
        if s.ui_remaining
            .is_some_and(|n| n < ((systems.len() - 1) * 2 + 10) as u64)
        {
            return Err(Error::RouteBudget);
        }
        let c = RouteConfirmation {
            id: oauth::random()?.to_string(),
            character: s
                .session
                .as_ref()
                .ok_or(Error::Disconnected)?
                .character
                .clone(),
            systems: systems.to_vec(),
            expires_at: (now() + 30).min(fresh),
        };
        let revision = s.location_revision;
        s.confirmation = None;
        s.error = None;
        s.route_confirmation = Some(Pending {
            confirmation: c.clone(),
            generation,
            revision,
        });
        Ok(c)
    }
    pub fn cancel_route(&self, id: &str) -> Result<(), Error> {
        let mut s = self.state()?;
        if id.len() != 43 {
            return Err(Error::InvalidConfirmation);
        }
        if s.active_route.as_deref() == Some(id) {
            s.active_route = None;
            return Ok(());
        }
        if s.route_confirmation
            .as_ref()
            .is_none_or(|p| p.confirmation.id != id)
        {
            return Err(Error::InvalidConfirmation);
        }
        s.route_confirmation = None;
        Ok(())
    }
    pub fn send_route(&self, root: &Path, id: &str) -> Result<Report, Error> {
        self.send_route_with(id, || Topology::load(root), std::thread::sleep, WORK)
    }
    fn send_route_with(
        &self,
        id: &str,
        mut load: impl FnMut() -> Result<Topology, Error>,
        mut pause: impl FnMut(Duration),
        budget: Duration,
    ) -> Result<Report, Error> {
        let _flight = self.flight()?;
        let start = Instant::now();
        let p = {
            let mut s = self.state()?;
            let p = s
                .route_confirmation
                .take()
                .ok_or(Error::InvalidConfirmation)?;
            if id.len() != 43 || p.confirmation.id != id || p.confirmation.expires_at <= now() {
                return Err(Error::InvalidConfirmation);
            }
            s.active_route = Some(id.to_owned());
            p
        };
        struct Active<'a>(&'a Eve, String);
        impl Drop for Active<'_> {
            fn drop(&mut self) {
                if let Ok(mut s) = self.0.state() {
                    if s.active_route.as_ref() == Some(&self.1) {
                        s.active_route = None;
                    }
                }
            }
        }
        let _active = Active(self, id.to_owned());
        let systems = &p.confirmation.systems;
        bounds(systems)?;
        let mut report = Report {
            delivery: Delivery::Stopped,
            total: systems.len() - 1,
            accepted_count: 0,
            last_accepted: None,
            failed_system: None,
            next_unsent: Some(systems[1]),
            reason: None,
        };
        for (index, &destination) in systems[1..].iter().enumerate() {
            if index > 0 {
                for _ in 0..30 {
                    if self.credentials().is_err()
                        || !self.current(p.generation)
                        || self
                            .state()?
                            .active_route
                            .as_deref()
                            .is_none_or(|v| v != id)
                    {
                        break;
                    }
                    pause(PACE / 30);
                }
            }
            let check = (|| {
                if start.elapsed().saturating_add(Duration::from_secs(20)) > budget {
                    return Err(Error::RouteTime);
                }
                let (generation, access, character) = self.credentials()?;
                if generation != p.generation || character != p.confirmation.character.id {
                    return Err(Error::Cancelled);
                }
                load()?.validate(systems)?;
                if index == 0 {
                    self.origin(generation, character, systems[0], Some(p.revision))?;
                }
                let s = self.state()?;
                if s.active_route.as_deref() != Some(id) {
                    return Err(Error::Cancelled);
                }
                if s.location_revision != p.revision {
                    return Err(Error::InvalidConfirmation);
                }
                let needed = ((report.total - index) * 2 + 10) as u64;
                if s.ui_remaining.is_some_and(|n| n < needed) {
                    return Err(Error::RouteBudget);
                }
                if index > 0 && s.ui_remaining.is_none() {
                    return Err(Error::RouteBudget);
                }
                if start.elapsed().saturating_add(Duration::from_secs(20)) > budget {
                    return Err(Error::RouteTime);
                }
                Ok(access)
            })();
            let access = match check {
                Ok(a) => a,
                Err(e) => {
                    report.reason = Some(e);
                    return Ok(report);
                }
            };
            // Consumed nonce, serial requests, no automatic retries or rollback.
            let response = self.inner.http.send(Request {
                target: Target::Waypoint {
                    destination,
                    replace: index == 0,
                },
                bearer: Some(access),
            });
            report.next_unsent = systems.get(index + 2).copied();
            match response {
                Err(e) => {
                    report.failed_system = Some(destination);
                    report.delivery = Delivery::Unknown;
                    report.reason = Some(
                        if matches!(
                            e,
                            Error::Unavailable | Error::Timeout | Error::InvalidResponse
                        ) {
                            Error::OutcomeUnknown
                        } else {
                            e
                        },
                    );
                    return Ok(report);
                }
                Ok(r) => {
                    self.limits(p.generation, &r);
                    // Unknown/missing UI headroom is not permission to burst.
                    if r.headers.get("x-ratelimit-group").is_none_or(|g| g != "ui") {
                        if let Ok(mut s) = self.state() {
                            if s.generation == p.generation {
                                s.ui_remaining = None;
                            }
                        }
                    }
                    if r.status != 204 {
                        let e = if r.status >= 500 {
                            Error::OutcomeUnknown
                        } else {
                            status_error(r.status)
                        };
                        self.fail(p.generation, e);
                        report.reason = Some(e);
                        report.failed_system = Some(destination);
                        report.delivery = if e == Error::OutcomeUnknown {
                            Delivery::Unknown
                        } else {
                            Delivery::Stopped
                        };
                        return Ok(report);
                    }
                    report.accepted_count += 1;
                    report.last_accepted = Some(destination);
                }
            }
        }
        report.delivery = Delivery::AllAccepted;
        Ok(report)
    }
}
#[cfg(test)]
mod tests;
