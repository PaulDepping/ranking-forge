use std::time::Instant;

use tracing::instrument;

use super::queries::{
    EntrantPage, EventEntrantsData, EventEntrantsVars, EventPhasesData, EventPhasesVars,
    EventSetsData, EventSetsVars, GameNode, GameSearchData, GameSearchVars, PhaseNode, SetPage,
    TournamentPage, TournamentsByUserData, TournamentsByUserVars, UserBySlugData, UserBySlugVars,
    UserNode,
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

                let phases = data.event.map(|e| e.phases).unwrap_or_default();

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
