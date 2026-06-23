use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, instrument};

use crate::api_types::{GqlError, GqlResponse};

pub const STARTGG_API_URL: &str = "https://api.start.gg/gql/alpha";

#[derive(Debug, thiserror::Error)]
#[error("query complexity too high{}", .actual.map(|n| format!(" (actual: {n})")).unwrap_or_default())]
pub struct ComplexityError {
    pub actual: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
#[error("API pagination limit reached (>10,000 entries)")]
pub struct PaginationLimitError;

#[derive(Debug, thiserror::Error)]
#[error("max retries exceeded for GraphQL query")]
pub struct MaxRetriesError;

fn parse_complexity_error(errors: &[GqlError]) -> Option<ComplexityError> {
    let msg = errors.first()?.message.as_deref()?;
    if !msg.contains("query complexity is too high") {
        return None;
    }
    let actual = msg
        .split("actual: ")
        .nth(1)
        .and_then(|s| s.trim_end_matches(')').trim().parse().ok());
    Some(ComplexityError { actual })
}

pub const TOURNAMENT_QUERY: &str = r#"
query Tournaments($page: Int!, $perPage: Int!, $filter: TournamentPageFilter) {
  tournaments(query: {
    page: $page
    perPage: $perPage
    filter: $filter
  }) {
    pageInfo { total totalPages }
    nodes {
      id name slug startAt endAt countryCode city addrState
      numAttendees isOnline lat lng timezone
      events {
        id name slug startAt state isOnline numEntrants type competitionTier
        videogame { id name }
      }
    }
  }
}
"#;

pub const PHASE_GROUPS_QUERY: &str = r#"
query EventPhaseGroups($eventId: ID!) {
  event(id: $eventId) {
    phases {
      id
      phaseGroups(query: { page: 1, perPage: 500 }) {
        pageInfo { totalPages }
        nodes { id }
      }
    }
  }
}
"#;

pub const PHASE_GROUP_SETS_QUERY: &str = r#"
query PhaseGroupSets($phaseGroupId: ID!, $page: Int!, $perPage: Int!) {
  phaseGroup(id: $phaseGroupId) {
    id
    sets(page: $page, perPage: $perPage) {
      pageInfo { total totalPages }
      nodes {
        id state winnerId vodUrl completedAt fullRoundText round
        lPlacement wPlacement displayScore
        phaseGroup {
          id displayIdentifier bracketType
          phase { id name bracketType phaseOrder isExhibition }
        }
        slots {
          slotIndex
          standing { stats { score { value } } }
          entrant {
            id initialSeedNum isDisqualified
            participants {
              player { id gamerTag prefix }
              user {
                id slug name bio genderPronoun
                location { city state country }
                images { url type }
              }
            }
          }
        }
        games {
          orderNum winnerId
          stage { id name }
          selections {
            selectionType
            entrant { id }
            character { id name }
          }
        }
      }
    }
  }
}
"#;

pub const PHASE_GROUP_SETS_QUERY_SLIM: &str = r#"
query PhaseGroupSets($phaseGroupId: ID!, $page: Int!, $perPage: Int!) {
  phaseGroup(id: $phaseGroupId) {
    id
    sets(page: $page, perPage: $perPage) {
      pageInfo { total totalPages }
      nodes {
        id state winnerId vodUrl completedAt fullRoundText round
        lPlacement wPlacement displayScore
        phaseGroup {
          id displayIdentifier bracketType
          phase { id name bracketType phaseOrder isExhibition }
        }
        slots {
          slotIndex
          standing { stats { score { value } } }
          entrant {
            id initialSeedNum isDisqualified
            participants {
              player { id gamerTag prefix }
            }
          }
        }
      }
    }
  }
}
"#;

pub const PHASE_GROUP_GAMES_QUERY: &str = r#"
query PhaseGroupGames($phaseGroupId: ID!, $page: Int!, $perPage: Int!) {
  phaseGroup(id: $phaseGroupId) {
    id
    sets(page: $page, perPage: $perPage) {
      pageInfo { total totalPages }
      nodes {
        id
        games {
          orderNum winnerId
          stage { id name }
          selections {
            selectionType
            entrant { id }
            character { id name }
          }
        }
      }
    }
  }
}
"#;

pub const EVENT_STANDINGS_QUERY: &str = r#"
query EventStandings($eventId: ID!, $page: Int!, $perPage: Int!) {
  event(id: $eventId) {
    standings(query: { page: $page, perPage: $perPage }) {
      pageInfo { total totalPages }
      nodes {
        id placement isFinal totalPoints
        entrant { id }
      }
    }
  }
}
"#;

#[instrument(skip(client, token, query), fields(%variables))]
pub async fn gql_query<T: DeserializeOwned>(
    client: &Client,
    base_url: &str,
    token: &str,
    query: &str,
    variables: Value,
    fallback_backoff: Duration,
) -> Result<T> {
    debug!("sending GraphQL request");
    let mut backoff = fallback_backoff;
    let mut last_decode_failure: Option<String> = None;
    for attempt in 0..15u32 {
        let resp = match client
            .post(base_url)
            .bearer_auth(token)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!(attempt = attempt + 1, error = %e, sleep_ms = backoff.as_millis(), "Request failed, retrying");
                sleep(backoff).await;
                continue;
            }
        };

        let status = resp.status();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs);

        if status.is_server_error() {
            let sleep_dur = retry_after.unwrap_or(backoff);
            debug!(attempt = attempt + 1, %status, sleep_ms = sleep_dur.as_millis(), "Server error, retrying");
            sleep(sleep_dur).await;
            continue;
        }

        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                let sleep_dur = retry_after.unwrap_or(backoff);
                debug!(attempt = attempt + 1, error = %e, sleep_ms = sleep_dur.as_millis(), "Failed to read body, retrying");
                sleep(sleep_dur).await;
                continue;
            }
        };

        let body: GqlResponse<T> = match serde_json::from_str(&text) {
            Ok(b) => b,
            Err(e) => {
                let sleep_dur = retry_after.unwrap_or(backoff);
                debug!(attempt = attempt + 1, error = %e, body = %text, sleep_ms = sleep_dur.as_millis(), "Failed to decode body, retrying");
                last_decode_failure = Some(text);
                sleep(sleep_dur).await;
                continue;
            }
        };

        if status == 429 {
            let sleep_dur = retry_after.unwrap_or(backoff);
            debug!(
                attempt = attempt + 1,
                sleep_ms = sleep_dur.as_millis(),
                "Rate limited, retrying"
            );
            sleep(sleep_dur).await;
            backoff = backoff.mul_f32(1.25);
            continue;
        }

        if let Some(errors) = &body.errors {
            if let Some(ce) = parse_complexity_error(errors) {
                return Err(anyhow::Error::new(ce));
            }
            if errors.iter().any(|e| {
                e.message
                    .as_deref()
                    .map(|m| m.contains("Cannot query more than the 10,000th entry"))
                    .unwrap_or(false)
            }) {
                return Err(anyhow::Error::new(PaginationLimitError));
            }
            let is_internal = errors.iter().any(|e| {
                e.extensions
                    .as_ref()
                    .and_then(|ext| ext.category.as_deref())
                    == Some("internal")
            });
            if is_internal {
                let sleep_dur = retry_after.unwrap_or(backoff);
                debug!(
                    attempt = attempt + 1,
                    sleep_ms = sleep_dur.as_millis(),
                    "Internal API error, retrying"
                );
                sleep(sleep_dur).await;
                continue;
            }
            anyhow::bail!("Unknown GraphQL errors in response");
        }

        if body.success == Some(false) {
            anyhow::bail!("GraphQL response reported success=false");
        }

        debug!(%status, "request succeeded");
        return body.data.context("GraphQL response missing data field");
    }

    if let Some(body) = last_decode_failure {
        tracing::error!(body, "Last response body before giving up");
    }
    Err(anyhow::Error::new(MaxRetriesError)
        .context(format!("Max retries exceeded (variables={})", variables)))
}
