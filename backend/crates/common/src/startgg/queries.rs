use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

// ── GraphQL envelope ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct GqlRequest<V: Serialize> {
    pub query: &'static str,
    pub variables: V,
}

#[derive(Deserialize)]
pub(crate) struct GqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GqlError>>,
}

#[derive(Deserialize)]
pub(crate) struct GqlError {
    pub message: String,
}

// ── Shared ────────────────────────────────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub total: Option<i32>,
    pub total_pages: Option<i32>,
}

// ── Game search ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct GameSearchVars {
    pub name: String,
}

#[derive(Deserialize)]
pub(crate) struct GameSearchData {
    pub videogames: Videogames,
}

#[derive(Deserialize)]
pub(crate) struct Videogames {
    pub nodes: Vec<GameNode>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameNode {
    pub id: i64,
    pub name: String,
    pub display_name: Option<String>,
}

// ── User by slug ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct UserBySlugVars {
    pub slug: String,
}

#[derive(Deserialize)]
pub(crate) struct UserBySlugData {
    pub user: Option<UserNode>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UserNode {
    pub id: i64,
    pub name: String,
}

// ── Tournaments by user ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct TournamentsByUserVars {
    #[serde(rename = "userId")]
    pub user_id: i64,
    #[serde(rename = "gameId")]
    pub game_id: i64,
    pub page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
}

#[derive(Deserialize)]
pub(crate) struct TournamentsByUserData {
    pub user: Option<UserWithTournaments>,
}

#[derive(Deserialize)]
pub(crate) struct UserWithTournaments {
    pub tournaments: TournamentPage,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TournamentPage {
    pub page_info: Option<PageInfo>,
    pub nodes: Vec<TournamentNode>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TournamentNode {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub city: Option<String>,
    pub addr_state: Option<String>,
    pub country_code: Option<String>,
    pub venue_name: Option<String>,
    pub venue_address: Option<String>,
    pub timezone: Option<String>,
    pub is_online: Option<bool>,
    pub num_attendees: Option<i32>,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
    pub events: Option<Vec<EventNode>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventNode {
    pub id: i64,
    pub name: String,
    pub num_entrants: Option<i32>,
    pub start_at: Option<i64>,
}

// ── Event entrants ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct EventEntrantsVars {
    #[serde(rename = "eventId")]
    pub event_id: i64,
    pub page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
}

#[derive(Deserialize)]
pub(crate) struct EventEntrantsData {
    pub event: Option<EventWithEntrants>,
}

#[derive(Deserialize)]
pub(crate) struct EventWithEntrants {
    pub entrants: EntrantPage,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntrantPage {
    pub page_info: Option<PageInfo>,
    pub nodes: Vec<EntrantNode>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntrantNode {
    pub id: i64,
    pub initial_seed_num: Option<i32>,
    pub is_disqualified: Option<bool>,
    pub standing: Option<EntrantStanding>,
    pub participants: Vec<Participant>,
}

impl EntrantNode {
    /// Display name from the first participant's gamer tag (singles events).
    pub fn display_name(&self) -> String {
        self.participants
            .first()
            .map(|p| p.gamer_tag.clone())
            .unwrap_or_default()
    }

    /// start.gg user ID from the first participant.
    pub fn startgg_user_id(&self) -> Option<i64> {
        self.participants.first()?.user.as_ref().map(|u| u.id)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct EntrantStanding {
    pub placement: Option<i32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub gamer_tag: String,
    pub user: Option<ParticipantUser>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ParticipantUser {
    pub id: i64,
}

// ── Event sets ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct EventSetsVars {
    #[serde(rename = "eventId")]
    pub event_id: i64,
    pub page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
}

#[derive(Deserialize)]
pub(crate) struct EventSetsData {
    pub event: Option<EventWithSets>,
}

#[derive(Deserialize)]
pub(crate) struct EventWithSets {
    pub sets: SetPage,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetPage {
    pub page_info: Option<PageInfo>,
    pub nodes: Vec<SetNode>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetNode {
    pub id: i64,
    pub winner_id: Option<i64>,
    pub round: Option<i32>,
    pub full_round_text: Option<String>,
    pub best_of: Option<i32>,
    pub completed_at: Option<i64>,
    pub vod_url: Option<String>,
    pub slots: Vec<SetSlot>,
}

impl SetNode {
    /// Whether this set was a DQ (score of -1 in any slot).
    pub fn is_dq(&self) -> bool {
        self.slots.iter().any(|slot| {
            slot.standing
                .as_ref()
                .and_then(|s| s.stats.as_ref())
                .and_then(|s| s.score.as_ref())
                .and_then(|s| s.value)
                .map(|v| v < 0.0)
                .unwrap_or(false)
        })
    }

    /// start.gg entrant ID of the loser.
    pub fn loser_id(&self) -> Option<i64> {
        let winner = self.winner_id?;
        self.slots.iter().find_map(|slot| {
            let eid = slot.entrant.as_ref()?.id;
            if eid != winner { Some(eid) } else { None }
        })
    }

    /// Score of winner and loser (games won), as i16 for DB storage.
    pub fn scores(&self) -> (Option<i16>, Option<i16>) {
        let winner = match self.winner_id {
            Some(id) => id,
            None => return (None, None),
        };
        let mut winner_score = None;
        let mut loser_score = None;
        for slot in &self.slots {
            let Some(entrant) = &slot.entrant else {
                continue;
            };
            let score = slot
                .standing
                .as_ref()
                .and_then(|s| s.stats.as_ref())
                .and_then(|s| s.score.as_ref())
                .and_then(|s| s.value)
                .map(|v| v as i16);
            if entrant.id == winner {
                winner_score = score;
            } else {
                loser_score = score;
            }
        }
        (winner_score, loser_score)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SetSlot {
    pub entrant: Option<SlotEntrant>,
    pub standing: Option<SlotStanding>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SlotEntrant {
    pub id: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SlotStanding {
    pub stats: Option<SlotStats>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SlotStats {
    pub score: Option<ScoreValue>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ScoreValue {
    pub value: Option<f64>,
}
