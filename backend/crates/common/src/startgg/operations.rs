use super::queries::{
    EntrantPage, EventEntrantsData, EventEntrantsVars, EventSetsData, EventSetsVars,
    GameNode, GameSearchData, GameSearchVars, SetPage, TournamentPage, TournamentsByUserData,
    TournamentsByUserVars, UserBySlugData, UserBySlugVars, UserNode,
};
use super::{StartggClient, StartggError};

const GAME_SEARCH_QUERY: &str = r#"
    query($name: String) {
        videogames(query: { name: $name }) {
            nodes { id name displayName }
        }
    }"#;

const USER_BY_SLUG_QUERY: &str =
    "query($slug: String) { user(slug: $slug) { id name } }";

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
                    startAt endAt
                    events(filter: { videogameId: [$gameId] }) {
                        id name numEntrants startAt
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
                    id winnerId round fullRoundText bestOf
                    completedAt vodUrl
                    slots {
                        entrant { id }
                        standing { stats { score { value } } }
                    }
                }
            }
        }
    }"#;

impl StartggClient {
    pub async fn search_games(&self, name: &str) -> Result<Vec<GameNode>, StartggError> {
        let data: GameSearchData = self
            .gql(GAME_SEARCH_QUERY, GameSearchVars { name: name.to_string() })
            .await?;
        Ok(data.videogames.nodes)
    }

    pub async fn user_by_slug(&self, slug: &str) -> Result<Option<UserNode>, StartggError> {
        let data: UserBySlugData = self
            .gql(USER_BY_SLUG_QUERY, UserBySlugVars { slug: slug.to_string() })
            .await?;
        Ok(data.user)
    }

    pub async fn tournaments_by_user(
        &self,
        user_id: i64,
        game_id: i64,
        page: i32,
        per_page: i32,
    ) -> Result<TournamentPage, StartggError> {
        let data: TournamentsByUserData = self
            .gql(
                TOURNAMENTS_BY_USER_QUERY,
                TournamentsByUserVars { user_id, game_id, page, per_page },
            )
            .await?;
        Ok(data
            .user
            .map(|u| u.tournaments)
            .unwrap_or_else(|| TournamentPage { page_info: None, nodes: vec![] }))
    }

    pub async fn event_entrants(
        &self,
        event_id: i64,
        page: i32,
        per_page: i32,
    ) -> Result<EntrantPage, StartggError> {
        let data: EventEntrantsData = self
            .gql(
                EVENT_ENTRANTS_QUERY,
                EventEntrantsVars { event_id, page, per_page },
            )
            .await?;
        Ok(data
            .event
            .map(|e| e.entrants)
            .unwrap_or_else(|| EntrantPage { page_info: None, nodes: vec![] }))
    }

    pub async fn event_sets(
        &self,
        event_id: i64,
        page: i32,
        per_page: i32,
    ) -> Result<SetPage, StartggError> {
        let data: EventSetsData = self
            .gql(
                EVENT_SETS_QUERY,
                EventSetsVars { event_id, page, per_page },
            )
            .await?;
        Ok(data
            .event
            .map(|e| e.sets)
            .unwrap_or_else(|| SetPage { page_info: None, nodes: vec![] }))
    }
}
