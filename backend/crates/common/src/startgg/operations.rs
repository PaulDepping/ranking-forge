use std::time::Instant;

use tracing::instrument;

use super::queries::{
    EntrantPage, EventEntrantsData, EventEntrantsVars, EventPhasesData, EventPhasesVars,
    EventSetsData, EventSetsVars, GameNode, GameSearchData, GameSearchVars, PhaseNode, SetPage,
    TournamentEntrant, TournamentEntrantListData, TournamentEntrantListVars, TournamentEventsData,
    TournamentEventsVars, TournamentPage, TournamentsByUserData, TournamentsByUserVars,
    UserBySlugData, UserBySlugVars, UserNode,
};
use super::{StartggClient, StartggError};

const GAME_SEARCH_QUERY: &str = r#"
    query($name: String) {
        videogames(query: { filter: { name: $name } }) {
            nodes { id name displayName }
        }
    }"#;

const USER_BY_SLUG_QUERY: &str =
    "query($slug: String) { user(slug: $slug) { id player { gamerTag } } }";

const TOURNAMENTS_BY_USER_QUERY: &str = r#"
    query($userId: ID!, $gameId: ID!, $page: Int!, $perPage: Int!) {
        user(id: $userId) {
            tournaments(query: {
                page: $page
                perPage: $perPage
                filter: { videogameId: [$gameId] }
            }) {
                pageInfo { total totalPages }
                nodes {
                    id name slug
                    city addrState countryCode
                    venueName venueAddress
                    timezone isOnline numAttendees
                    lat lng state
                    startAt endAt
                    events(filter: { videogameId: [$gameId] }) {
                        id name numEntrants startAt
                        slug state isOnline type
                        teamRosterSize { minPlayers maxPlayers }
                    }
                }
            }
        }
    }"#;

const EVENT_ENTRANTS_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            entrants(query: { page: $page, perPage: $perPage }) {
                pageInfo { total totalPages }
                nodes {
                    id initialSeedNum isDisqualified
                    standing { placement }
                    participants { gamerTag user { id } }
                }
            }
        }
    }"#;

const EVENT_SETS_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            sets(page: $page, perPage: $perPage, sortType: STANDARD) {
                pageInfo { total totalPages }
                nodes {
                    id winnerId round fullRoundText totalGames
                    completedAt vodUrl
                    hasPlaceholder state identifier
                    phaseGroup { id }
                    slots {
                        entrant { id }
                        standing { stats { score { value } } }
                    }
                }
            }
        }
    }"#;

const EVENT_PHASES_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            phases {
                id name bracketType phaseOrder
                numSeeds groupCount state isExhibition
                phaseGroups(query: { page: $page, perPage: $perPage }) {
                    pageInfo { total totalPages }
                    nodes {
                        id displayIdentifier bracketType bracketUrl
                        numRounds startAt firstRoundTime state
                    }
                }
            }
        }
    }"#;

const TOURNAMENT_EVENTS_QUERY: &str = r#"
    query($slug: String!, $gameId: ID!) {
        tournament(slug: $slug) {
            events(filter: { videogameId: [$gameId] }) {
                id
            }
        }
    }"#;

const TOURNAMENT_ENTRANT_LIST_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            entrants(query: { page: $page, perPage: $perPage }) {
                pageInfo { totalPages }
                nodes {
                    participants {
                        gamerTag
                        user { id slug }
                    }
                }
            }
        }
    }"#;

impl StartggClient {
    #[instrument(skip(self))]
    pub async fn search_games(&self, name: &str) -> Result<Vec<GameNode>, StartggError> {
        let t = Instant::now();
        let data: GameSearchData = self
            .gql(
                GAME_SEARCH_QUERY,
                GameSearchVars {
                    name: name.to_string(),
                },
            )
            .await?;
        tracing::debug!(
            elapsed_ms = t.elapsed().as_millis(),
            "startgg query complete"
        );
        Ok(data.videogames.nodes)
    }

    #[instrument(skip(self))]
    pub async fn user_by_slug(&self, slug: &str) -> Result<Option<UserNode>, StartggError> {
        let t = Instant::now();
        let data: UserBySlugData = self
            .gql(
                USER_BY_SLUG_QUERY,
                UserBySlugVars {
                    slug: slug.to_string(),
                },
            )
            .await?;
        tracing::debug!(
            elapsed_ms = t.elapsed().as_millis(),
            "startgg query complete"
        );
        Ok(data.user)
    }

    #[instrument(skip(self))]
    pub async fn tournaments_by_user(
        &self,
        user_id: i64,
        game_id: i64,
        page: i32,
        per_page: i32,
    ) -> Result<TournamentPage, StartggError> {
        let t = Instant::now();
        let data: TournamentsByUserData = self
            .gql(
                TOURNAMENTS_BY_USER_QUERY,
                TournamentsByUserVars {
                    user_id,
                    game_id,
                    page,
                    per_page,
                },
            )
            .await?;
        tracing::debug!(
            elapsed_ms = t.elapsed().as_millis(),
            "startgg query complete"
        );
        Ok(data
            .user
            .map(|u| u.tournaments)
            .unwrap_or_else(|| TournamentPage {
                page_info: None,
                nodes: vec![],
            }))
    }

    #[instrument(skip(self))]
    pub async fn event_entrants(
        &self,
        event_id: i64,
        page: i32,
        per_page: i32,
    ) -> Result<EntrantPage, StartggError> {
        let t = Instant::now();
        let data: EventEntrantsData = self
            .gql(
                EVENT_ENTRANTS_QUERY,
                EventEntrantsVars {
                    event_id,
                    page,
                    per_page,
                },
            )
            .await?;
        tracing::debug!(
            elapsed_ms = t.elapsed().as_millis(),
            "startgg query complete"
        );
        Ok(data
            .event
            .map(|e| e.entrants)
            .unwrap_or_else(|| EntrantPage {
                page_info: None,
                nodes: vec![],
            }))
    }

    #[instrument(skip(self))]
    pub async fn event_sets(
        &self,
        event_id: i64,
        page: i32,
        per_page: i32,
    ) -> Result<SetPage, StartggError> {
        let t = Instant::now();
        let data: EventSetsData = self
            .gql(
                EVENT_SETS_QUERY,
                EventSetsVars {
                    event_id,
                    page,
                    per_page,
                },
            )
            .await?;
        tracing::debug!(
            elapsed_ms = t.elapsed().as_millis(),
            "startgg query complete"
        );
        Ok(data.event.map(|e| e.sets).unwrap_or_else(|| SetPage {
            page_info: None,
            nodes: vec![],
        }))
    }

    #[instrument(skip(self))]
    pub async fn tournament_entrants(
        &self,
        tournament_handle: &str,
        game_id: i64,
    ) -> Result<Vec<TournamentEntrant>, StartggError> {
        let t = Instant::now();

        // Query 1: get event IDs for this tournament filtered by game.
        let events_data: TournamentEventsData = self
            .gql(
                TOURNAMENT_EVENTS_QUERY,
                TournamentEventsVars {
                    slug: tournament_handle.to_string(),
                    game_id,
                },
            )
            .await?;

        let event_ids: Vec<i64> = events_data
            .tournament
            .and_then(|t| t.events)
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect();

        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for event_id in event_ids {
            let mut page = 1i32;
            let mut per_page = 64i32;

            'pages: loop {
                let data: TournamentEntrantListData = match self
                    .gql(
                        TOURNAMENT_ENTRANT_LIST_QUERY,
                        TournamentEntrantListVars { event_id, page, per_page },
                    )
                    .await
                {
                    Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                        tracing::warn!(per_page, actual, limit, "complexity too high, halving perPage");
                        per_page /= 2;
                        continue 'pages;
                    }
                    other => other?,
                };

                let entrant_data = match data.event.and_then(|e| e.entrants) {
                    Some(d) => d,
                    None => break,
                };

                for node in entrant_data.nodes {
                    let Some(participants) = node.participants else {
                        continue;
                    };
                    for participant in participants {
                        let Some(user) = participant.user else {
                            continue;
                        };
                        if seen.insert(user.id) {
                            let handle = user.slug.trim_start_matches("user/").to_string();
                            result.push(TournamentEntrant {
                                startgg_user_id: user.id,
                                handle,
                                name: participant.gamer_tag,
                            });
                        }
                    }
                }

                let total_pages = entrant_data.page_info.total_pages.unwrap_or(1);
                if page >= total_pages {
                    break;
                }
                page += 1;
            }
        }

        tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "startgg query complete");
        Ok(result)
    }

    #[instrument(skip(self))]
    pub async fn event_phases(&self, event_id: i64) -> Result<Vec<PhaseNode>, StartggError> {
        let t = Instant::now();
        let mut per_page = 25i32;

        let result = 'pages: loop {
            let mut phases_acc: Vec<PhaseNode> = Vec::new();
            let mut page = 1i32;

            loop {
                let data: EventPhasesData = match self
                    .gql(EVENT_PHASES_QUERY, EventPhasesVars { event_id, page, per_page })
                    .await
                {
                    Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                        tracing::warn!(per_page, actual, limit, "complexity too high, halving perPage");
                        per_page /= 2;
                        continue 'pages;
                    }
                    other => other?,
                };

                let phases = data.event.and_then(|e| e.phases).unwrap_or_default();

                let max_total_pages = phases
                    .iter()
                    .filter_map(|p| p.phase_groups.as_ref())
                    .filter_map(|pg| pg.page_info.as_ref())
                    .filter_map(|pi| pi.total_pages)
                    .max()
                    .unwrap_or(1);

                if page == 1 {
                    phases_acc = phases;
                } else {
                    for phase in phases {
                        if let Some(existing) = phases_acc.iter_mut().find(|p| p.id == phase.id) {
                            if let (Some(existing_pg), Some(new_pg)) =
                                (existing.phase_groups.as_mut(), phase.phase_groups)
                            {
                                existing_pg.nodes.extend(new_pg.nodes);
                            }
                        }
                    }
                }

                if page >= max_total_pages {
                    break 'pages phases_acc;
                }
                page += 1;
            }
        };

        tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "startgg query complete");
        Ok(result)
    }
}
