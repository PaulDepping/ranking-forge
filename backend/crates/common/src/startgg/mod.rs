mod operations;
mod queries;

pub use queries::{
    EntrantNode, EntrantPage, EntrantStanding, EventNode, GameNode, PageInfo, Participant,
    ParticipantUser, PhaseGroupNode, PhaseGroupPage, PhaseNode, ScoreValue, SetNode, SetPage,
    SetPhaseGroup, SetSlot, SlotEntrant, SlotStanding, SlotStats, TeamRosterSize, TournamentEntrant,
    TournamentNode, TournamentPage, UserNode,
};

use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use thiserror::Error;

const STARTGG_BASE_URL: &str = "https://api.start.gg/gql/alpha";

#[derive(Debug, Error)]
pub enum StartggError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("GraphQL error: {0}")]
    GraphQL(String),
    #[error("query complexity too high (limit: {limit}, actual: {actual})")]
    ComplexityTooHigh { limit: u32, actual: u32 },
    #[error("response decode error: {0}")]
    Decode(String),
}

fn parse_complexity_error(msg: &str) -> Option<StartggError> {
    use regex::Regex;
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"A maximum of (\d+) objects may be returned.*\(actual: (\d+)\)").unwrap()
    });
    let caps = re.captures(msg)?;
    let limit = caps[1].parse::<u32>().ok()?;
    let actual = caps[2].parse::<u32>().ok()?;
    Some(StartggError::ComplexityTooHigh { limit, actual })
}

#[derive(Clone)]
pub struct StartggClient {
    http: Client,
    api_key: String,
    base_url: String,
    retry_min_delay: Duration,
}

impl StartggClient {
    pub fn new(api_key: String) -> Self {
        Self::new_with_base_url(api_key, STARTGG_BASE_URL.into())
    }

    pub fn new_with_base_url(api_key: String, base_url: String) -> Self {
        let http = Client::builder()
            .user_agent("rankingforge/0.1")
            .build()
            .expect("failed to build HTTP client");
        StartggClient {
            http,
            api_key,
            base_url,
            retry_min_delay: Duration::from_secs(1),
        }
    }

    pub fn with_retry_min_delay(mut self, d: Duration) -> Self {
        self.retry_min_delay = d;
        self
    }

    async fn gql<V, T>(&self, query: &'static str, variables: V) -> Result<T, StartggError>
    where
        V: Serialize,
        T: serde::de::DeserializeOwned,
    {
        use backon::{ExponentialBuilder, Retryable};
        use queries::{GqlRequest, GqlResponse};

        let vars =
            serde_json::to_value(variables).map_err(|e| StartggError::GraphQL(e.to_string()))?;

        (|| async {
            let body = self
                .http
                .post(&self.base_url)
                .bearer_auth(&self.api_key)
                .json(&GqlRequest {
                    query,
                    variables: &vars,
                })
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;

            let resp: GqlResponse<serde_json::Value> =
                serde_json::from_str(&body).map_err(|e| {
                    let preview: String = body.chars().take(500).collect();
                    tracing::error!(body = %preview, "failed to decode start.gg response: {e}");
                    StartggError::Decode(e.to_string())
                })?;

            if let Some(errors) = resp.errors {
                let msg = errors
                    .into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join("; ");
                if let Some(err) = parse_complexity_error(&msg) {
                    return Err(err);
                }
                tracing::error!(body = %body, "start.gg returned GraphQL errors: {msg}");
                return Err(StartggError::GraphQL(msg));
            }

            let data_value = resp
                .data
                .ok_or_else(|| StartggError::GraphQL("empty data field in response".into()))?;
            serde_json::from_value(data_value).map_err(|e| {
                tracing::error!("failed to decode start.gg data: {e}");
                StartggError::Decode(e.to_string())
            })
        })
        .retry(
            ExponentialBuilder::default()
                .with_min_delay(self.retry_min_delay)
                .with_max_delay(Duration::from_secs(60))
                .with_max_times(usize::MAX)
                .with_jitter(),
        )
        .when(|e| {
            matches!(e, StartggError::Http(re) if re.status()
                == Some(reqwest::StatusCode::TOO_MANY_REQUESTS))
        })
        .notify(|_err, dur| {
            tracing::debug!(?dur, "start.gg rate limited; retrying");
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(base_url: &str) -> StartggClient {
        StartggClient::new_with_base_url("test-key".into(), base_url.into())
            .with_retry_min_delay(std::time::Duration::from_millis(1))
    }

    fn mock_ok(body: serde_json::Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(body)
    }

    // ── search_games ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn search_games_returns_game_list() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "videogames": {
                        "nodes": [
                            {"id": 1, "name": "Super Smash Bros. Melee", "displayName": "SSBM"},
                            {"id": 2, "name": "Super Smash Bros. Ultimate", "displayName": null}
                        ]
                    }
                }
            })))
            .mount(&mock)
            .await;

        let games = client(&mock.uri()).search_games("smash").await.unwrap();

        assert_eq!(games.len(), 2);
        assert_eq!(games[0].id, 1);
        assert_eq!(games[0].name, "Super Smash Bros. Melee");
        assert_eq!(games[0].display_name.as_deref(), Some("SSBM"));
        assert_eq!(games[1].id, 2);
        assert!(games[1].display_name.is_none());
    }

    #[tokio::test]
    async fn search_games_returns_empty_list() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "videogames": { "nodes": [] } }
            })))
            .mount(&mock)
            .await;

        let games = client(&mock.uri()).search_games("zzz").await.unwrap();
        assert!(games.is_empty());
    }

    #[tokio::test]
    async fn search_games_surfaces_graphql_error() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": null,
                "errors": [{"message": "rate limit exceeded"}]
            })))
            .mount(&mock)
            .await;

        let err = client(&mock.uri()).search_games("melee").await.unwrap_err();
        assert!(matches!(err, StartggError::GraphQL(msg) if msg.contains("rate limit")));
    }

    #[tokio::test]
    async fn search_games_surfaces_http_error() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock)
            .await;

        let err = client(&mock.uri()).search_games("melee").await.unwrap_err();
        assert!(matches!(err, StartggError::Http(_)));
    }

    // ── user_by_slug ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn user_by_slug_returns_user_when_found() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "user": { "id": 12345, "player": { "gamerTag": "Mango" } } }
            })))
            .mount(&mock)
            .await;

        let user = client(&mock.uri())
            .user_by_slug("user/mango")
            .await
            .unwrap();
        let user = user.expect("expected Some(user)");
        assert_eq!(user.id, 12345);
        assert_eq!(user.gamer_tag(), Some("Mango"));
    }

    #[tokio::test]
    async fn user_by_slug_returns_none_when_not_found() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "user": null }
            })))
            .mount(&mock)
            .await;

        let user = client(&mock.uri())
            .user_by_slug("user/nobody")
            .await
            .unwrap();
        assert!(user.is_none());
    }

    #[tokio::test]
    async fn user_by_slug_surfaces_graphql_error() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": null,
                "errors": [{"message": "not authorized"}, {"message": "quota exceeded"}]
            })))
            .mount(&mock)
            .await;

        let err = client(&mock.uri())
            .user_by_slug("user/x")
            .await
            .unwrap_err();
        match err {
            StartggError::GraphQL(msg) => {
                assert!(msg.contains("not authorized"));
                assert!(msg.contains("quota exceeded"));
            }
            other => panic!("expected GraphQL error, got {other}"),
        }
    }

    // ── tournaments_by_user ───────────────────────────────────────────────────

    #[tokio::test]
    async fn tournaments_by_user_works_with_empty_result() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "user": { "tournaments": {
                    "pageInfo": { "total": 0, "totalPages": 1 },
                    "nodes": []
                }}}
            })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri())
            .tournaments_by_user(99, 1, 1, 25)
            .await
            .unwrap();
        assert!(page.nodes.is_empty());
    }

    #[tokio::test]
    async fn tournaments_by_user_returns_page() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "user": {
                        "tournaments": {
                            "pageInfo": { "total": 1, "totalPages": 1 },
                            "nodes": [{
                                "id": 100,
                                "name": "CEO 2024",
                                "slug": "tournament/ceo-2024",
                                "city": "Orlando",
                                "addrState": "FL",
                                "countryCode": "US",
                                "venueName": null,
                                "venueAddress": null,
                                "timezone": "America/New_York",
                                "isOnline": false,
                                "numAttendees": 500,
                                "startAt": 1700000000_i64,
                                "endAt": 1700086400_i64,
                                "events": [
                                    { "id": 200, "name": "Melee Singles", "numEntrants": 300, "startAt": 1700000000_i64 }
                                ]
                            }]
                        }
                    }
                }
            })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri())
            .tournaments_by_user(99, 1, 1, 25)
            .await
            .unwrap();

        assert_eq!(page.page_info.as_ref().unwrap().total, Some(1));
        assert_eq!(page.nodes.len(), 1);

        let t = &page.nodes[0];
        assert_eq!(t.id, 100);
        assert_eq!(t.name, "CEO 2024");
        assert_eq!(t.slug, "tournament/ceo-2024");
        assert_eq!(t.city.as_deref(), Some("Orlando"));
        assert_eq!(t.addr_state.as_deref(), Some("FL"));
        assert_eq!(t.country_code.as_deref(), Some("US"));
        assert_eq!(t.timezone.as_deref(), Some("America/New_York"));
        assert_eq!(t.is_online, Some(false));
        assert_eq!(t.num_attendees, Some(500));
        assert_eq!(t.start_at, Some(1700000000));

        let events = t.events.as_ref().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, 200);
        assert_eq!(events[0].name, "Melee Singles");
        assert_eq!(events[0].num_entrants, Some(300));
    }

    #[tokio::test]
    async fn tournaments_by_user_returns_empty_when_user_not_found() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "user": null }
            })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri())
            .tournaments_by_user(0, 1, 1, 25)
            .await
            .unwrap();
        assert!(page.nodes.is_empty());
        assert!(page.page_info.is_none());
    }

    // ── event_entrants ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn event_entrants_returns_page() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "entrants": {
                            "pageInfo": { "total": 2, "totalPages": 1 },
                            "nodes": [
                                {
                                    "id": 1001,
                                    "initialSeedNum": 1,
                                    "isDisqualified": false,
                                    "standing": { "placement": 1 },
                                    "participants": [{ "gamerTag": "Mango", "user": { "id": 12345 } }]
                                },
                                {
                                    "id": 1002,
                                    "initialSeedNum": 2,
                                    "isDisqualified": false,
                                    "standing": { "placement": 2 },
                                    "participants": [{ "gamerTag": "Armada", "user": { "id": 67890 } }]
                                }
                            ]
                        }
                    }
                }
            })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri())
            .event_entrants(200, 1, 25)
            .await
            .unwrap();

        assert_eq!(page.page_info.as_ref().unwrap().total, Some(2));
        assert_eq!(page.nodes.len(), 2);

        let e0 = &page.nodes[0];
        assert_eq!(e0.id, 1001);
        assert_eq!(e0.initial_seed_num, Some(1));
        assert_eq!(e0.display_name(), "Mango");
        assert_eq!(e0.startgg_user_id(), Some(12345));
        assert_eq!(e0.standing.as_ref().unwrap().placement, Some(1));

        let e1 = &page.nodes[1];
        assert_eq!(e1.id, 1002);
        assert_eq!(e1.display_name(), "Armada");
        assert_eq!(e1.startgg_user_id(), Some(67890));
    }

    #[tokio::test]
    async fn event_entrants_returns_empty_when_event_not_found() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({ "data": { "event": null } })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri()).event_entrants(0, 1, 25).await.unwrap();
        assert!(page.nodes.is_empty());
    }

    // ── event_sets ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn event_sets_returns_page() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "sets": {
                            "pageInfo": { "total": 1, "totalPages": 1 },
                            "nodes": [{
                                "id": 3001,
                                "winnerId": 1001,
                                "round": 6,
                                "fullRoundText": "Grand Final",
                                "totalGames": 5,
                                "completedAt": 1700050000_i64,
                                "vodUrl": null,
                                "slots": [
                                    {
                                        "entrant": { "id": 1001 },
                                        "standing": { "stats": { "score": { "value": 3.0 } } }
                                    },
                                    {
                                        "entrant": { "id": 1002 },
                                        "standing": { "stats": { "score": { "value": 1.0 } } }
                                    }
                                ]
                            }]
                        }
                    }
                }
            })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri()).event_sets(200, 1, 25).await.unwrap();

        assert_eq!(page.page_info.as_ref().unwrap().total, Some(1));
        assert_eq!(page.nodes.len(), 1);

        let s = &page.nodes[0];
        assert_eq!(s.id, 3001);
        assert_eq!(s.winner_id, Some(1001));
        assert_eq!(s.round, Some(6));
        assert_eq!(s.full_round_text.as_deref(), Some("Grand Final"));
        assert_eq!(s.total_games, Some(5));
        assert_eq!(s.completed_at, Some(1700050000));
        assert_eq!(s.loser_id(), Some(1002));
        assert!(!s.is_dq());
        assert_eq!(s.scores(), (Some(3), Some(1)));
    }

    #[tokio::test]
    async fn event_sets_detects_dq_from_negative_score() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "sets": {
                            "pageInfo": { "total": 1, "totalPages": 1 },
                            "nodes": [{
                                "id": 9999,
                                "winnerId": 1001,
                                "round": 1,
                                "fullRoundText": "Round 1",
                                "totalGames": 3,
                                "completedAt": null,
                                "vodUrl": null,
                                "slots": [
                                    {
                                        "entrant": { "id": 1001 },
                                        "standing": { "stats": { "score": { "value": 2.0 } } }
                                    },
                                    {
                                        "entrant": { "id": 1002 },
                                        "standing": { "stats": { "score": { "value": -1.0 } } }
                                    }
                                ]
                            }]
                        }
                    }
                }
            })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri()).event_sets(200, 1, 25).await.unwrap();
        assert!(page.nodes[0].is_dq());
    }

    #[tokio::test]
    async fn event_sets_returns_empty_when_event_not_found() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({ "data": { "event": null } })))
            .mount(&mock)
            .await;

        let page = client(&mock.uri()).event_sets(0, 1, 25).await.unwrap();
        assert!(page.nodes.is_empty());
    }

    // ── rate limit retry ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn rate_limited_once_then_succeeds() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .mount(&mock)
            .await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "videogames": { "nodes": [] } }
            })))
            .mount(&mock)
            .await;

        let games = client(&mock.uri()).search_games("melee").await.unwrap();
        assert!(games.is_empty());
    }

    #[tokio::test]
    async fn rate_limited_multiple_times_then_succeeds() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(3)
            .mount(&mock)
            .await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "videogames": { "nodes": [
                    {"id": 1, "name": "Melee", "displayName": null}
                ] } }
            })))
            .mount(&mock)
            .await;

        let games = client(&mock.uri()).search_games("melee").await.unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, 1);
    }

    #[tokio::test]
    async fn complexity_error_is_parsed_as_complexity_too_high() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": null,
                "errors": [{"message": "Your query complexity is too high. A maximum of 1000 objects may be returned by each request. (actual: 1203)"}]
            })))
            .mount(&mock)
            .await;

        let err = client(&mock.uri()).search_games("melee").await.unwrap_err();
        assert!(
            matches!(err, StartggError::ComplexityTooHigh { limit: 1000, actual: 1203 }),
            "expected ComplexityTooHigh, got {err:?}"
        );
    }

    #[tokio::test]
    async fn non_complexity_graphql_error_surfaces_as_graphql() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": null,
                "errors": [{"message": "not authorized"}]
            })))
            .mount(&mock)
            .await;

        let err = client(&mock.uri()).search_games("melee").await.unwrap_err();
        assert!(
            matches!(err, StartggError::GraphQL(_)),
            "expected GraphQL error, got {err:?}"
        );
    }

    // ── tournament_entrants ───────────────────────────────────────────────────

    #[tokio::test]
    async fn tournament_entrants_returns_entrants() {
        let mock = MockServer::start().await;

        // First request: fetch event IDs
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "tournament": {
                        "events": [
                            { "id": 101 },
                            { "id": 102 }
                        ]
                    }
                }
            })))
            .up_to_n_times(1)
            .mount(&mock)
            .await;

        // Second request: entrants for event 101
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "entrants": {
                            "pageInfo": { "totalPages": 1 },
                            "nodes": [
                                {
                                    "participants": [{
                                        "gamerTag": "Mang0",
                                        "user": { "id": 1001, "slug": "user/mang0" }
                                    }]
                                },
                                {
                                    "participants": [{
                                        "gamerTag": "Armada",
                                        "user": { "id": 1002, "slug": "user/armada" }
                                    }]
                                }
                            ]
                        }
                    }
                }
            })))
            .up_to_n_times(1)
            .mount(&mock)
            .await;

        // Third request: entrants for event 102 (Mang0 also in this event — dedup test)
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "entrants": {
                            "pageInfo": { "totalPages": 1 },
                            "nodes": [
                                {
                                    "participants": [{
                                        "gamerTag": "Mang0",
                                        "user": { "id": 1001, "slug": "user/mang0" }
                                    }]
                                },
                                {
                                    "participants": [{
                                        "gamerTag": "Leffen",
                                        "user": { "id": 1003, "slug": "user/leffen" }
                                    }]
                                }
                            ]
                        }
                    }
                }
            })))
            .up_to_n_times(1)
            .mount(&mock)
            .await;

        let entrants = client(&mock.uri())
            .tournament_entrants("some-weekly", 1)
            .await
            .unwrap();

        assert_eq!(entrants.len(), 3); // Mang0, Armada, Leffen (Mang0 deduplicated)
        let handles: Vec<&str> = entrants.iter().map(|e| e.handle.as_str()).collect();
        assert!(handles.contains(&"mang0"));
        assert!(handles.contains(&"armada"));
        assert!(handles.contains(&"leffen"));
    }

    #[tokio::test]
    async fn tournament_entrants_omits_guests() {
        let mock = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": { "tournament": { "events": [{ "id": 101 }] } }
            })))
            .up_to_n_times(1)
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "entrants": {
                            "pageInfo": { "totalPages": 1 },
                            "nodes": [
                                {
                                    "participants": [{
                                        "gamerTag": "Mang0",
                                        "user": { "id": 1001, "slug": "user/mang0" }
                                    }]
                                },
                                {
                                    "participants": [{
                                        "gamerTag": "GuestPlayer",
                                        "user": null
                                    }]
                                }
                            ]
                        }
                    }
                }
            })))
            .up_to_n_times(1)
            .mount(&mock)
            .await;

        let entrants = client(&mock.uri())
            .tournament_entrants("some-weekly", 1)
            .await
            .unwrap();

        assert_eq!(entrants.len(), 1);
        assert_eq!(entrants[0].handle, "mang0");
    }

    // ── event_phases ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn event_phases_merges_paginated_phase_groups() {
        let mock = MockServer::start().await;

        // Page 1 of phase groups: totalPages = 2
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "phases": [{
                            "id": 1,
                            "name": "Pools",
                            "bracketType": "DOUBLE_ELIMINATION",
                            "phaseOrder": 1,
                            "numSeeds": null,
                            "groupCount": 2,
                            "state": "COMPLETED",
                            "isExhibition": false,
                            "phaseGroups": {
                                "pageInfo": { "total": 2, "totalPages": 2 },
                                "nodes": [
                                    { "id": 100, "displayIdentifier": "1", "bracketType": "DOUBLE_ELIMINATION",
                                      "bracketUrl": null, "numRounds": null, "startAt": null,
                                      "firstRoundTime": null, "state": 3 }
                                ]
                            }
                        }]
                    }
                }
            })))
            .up_to_n_times(1)
            .mount(&mock)
            .await;

        // Page 2 of phase groups: totalPages = 2
        Mock::given(method("POST"))
            .respond_with(mock_ok(json!({
                "data": {
                    "event": {
                        "phases": [{
                            "id": 1,
                            "name": "Pools",
                            "bracketType": "DOUBLE_ELIMINATION",
                            "phaseOrder": 1,
                            "numSeeds": null,
                            "groupCount": 2,
                            "state": "COMPLETED",
                            "isExhibition": false,
                            "phaseGroups": {
                                "pageInfo": { "total": 2, "totalPages": 2 },
                                "nodes": [
                                    { "id": 101, "displayIdentifier": "2", "bracketType": "DOUBLE_ELIMINATION",
                                      "bracketUrl": null, "numRounds": null, "startAt": null,
                                      "firstRoundTime": null, "state": 3 }
                                ]
                            }
                        }]
                    }
                }
            })))
            .mount(&mock)
            .await;

        let phases = client(&mock.uri()).event_phases(200).await.unwrap();
        assert_eq!(phases.len(), 1);
        let groups = phases[0].phase_groups.as_ref().unwrap();
        assert_eq!(groups.nodes.len(), 2, "expected groups from both pages merged");
        assert_eq!(groups.nodes[0].id, 100);
        assert_eq!(groups.nodes[1].id, 101);
    }
}
