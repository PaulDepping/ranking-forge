use serde::{Deserialize, Deserializer};

// ---------------------------------------------------------------------------
// ID helpers — start.gg returns IDs as either integers or strings
// ---------------------------------------------------------------------------

pub fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub fn deserialize_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Int(i64),
        Str(String),
    }
    match Raw::deserialize(deserializer)? {
        Raw::Int(n) => Ok(n),
        Raw::Str(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

pub fn deserialize_opt_id<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Int(i64),
        Str(String),
        Null,
    }
    match Option::<Raw>::deserialize(deserializer)? {
        None | Some(Raw::Null) => Ok(None),
        Some(Raw::Int(n)) => Ok(Some(n)),
        Some(Raw::Str(s)) => Ok(s.parse().ok()),
    }
}

// ---------------------------------------------------------------------------
// GQL envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GqlError>>,
    pub success: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GqlError {
    pub message: Option<String>,
    pub extensions: Option<GqlErrorExtensions>,
}

#[derive(Debug, Deserialize)]
pub struct GqlErrorExtensions {
    pub category: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub total: Option<i64>,
    pub total_pages: Option<i64>,
}

// ---------------------------------------------------------------------------
// TOURNAMENT_QUERY
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TournamentsData {
    pub tournaments: TournamentsPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentsPage {
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<TournamentNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub addr_state: Option<String>,
    pub num_attendees: Option<i64>,
    pub is_online: Option<bool>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub timezone: Option<String>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub events: Vec<EventNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub start_at: Option<i64>,
    pub state: Option<String>,
    pub is_online: Option<bool>,
    pub num_entrants: Option<i64>,
    #[serde(rename = "type")]
    pub event_type: Option<i64>,
    pub competition_tier: Option<i64>,
    pub videogame: Option<VideogameNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideogameNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: String,
}

// ---------------------------------------------------------------------------
// PHASE_GROUPS_QUERY
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EventPhaseGroupsData {
    pub event: Option<EventPhasesNode>,
}

#[derive(Debug, Deserialize)]
pub struct EventPhasesNode {
    #[serde(deserialize_with = "deserialize_null_default")]
    pub phases: Vec<PhaseNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub phase_groups: PhaseGroupsPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseGroupsPage {
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<PhaseGroupIdNode>,
}

#[derive(Debug, Deserialize)]
pub struct PhaseGroupIdNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
}

// ---------------------------------------------------------------------------
// PHASE_GROUP_SETS_QUERY (full)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullPhaseGroupSetsData {
    pub phase_group: Option<FullPhaseGroupNode>,
}

#[derive(Debug, Deserialize)]
pub struct FullPhaseGroupNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub sets: SetsPage<FullSetNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetsPage<T>
where
    T: serde::de::DeserializeOwned,
{
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullSetNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub state: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub winner_id: Option<i64>,
    pub vod_url: Option<String>,
    pub completed_at: Option<i64>,
    pub full_round_text: Option<String>,
    pub round: Option<i64>,
    pub l_placement: Option<i64>,
    pub w_placement: Option<i64>,
    pub display_score: Option<String>,
    pub phase_group: Option<PhaseGroupInfo>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub slots: Vec<SlotNode>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub games: Vec<GameNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseGroupInfo {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub display_identifier: Option<String>,
    pub bracket_type: Option<String>,
    pub phase: Option<PhaseInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseInfo {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: Option<String>,
    pub bracket_type: Option<String>,
    pub phase_order: Option<i64>,
    pub is_exhibition: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotNode {
    pub slot_index: Option<i64>,
    pub standing: Option<SlotStanding>,
    pub entrant: Option<EntrantNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlotStanding {
    pub stats: Option<SlotStats>,
}

#[derive(Debug, Deserialize)]
pub struct SlotStats {
    pub score: Option<ScoreValue>,
}

#[derive(Debug, Deserialize)]
pub struct ScoreValue {
    pub value: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrantNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub initial_seed_num: Option<i64>,
    pub is_disqualified: Option<bool>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub participants: Vec<ParticipantNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantNode {
    pub player: Option<PlayerNode>,
    pub user: Option<UserNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub gamer_tag: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub gender_pronoun: Option<String>,
    pub location: Option<UserLocation>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub images: Vec<UserImage>,
}

#[derive(Debug, Deserialize)]
pub struct UserLocation {
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserImage {
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub image_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameNode {
    pub order_num: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub winner_id: Option<i64>,
    pub stage: Option<StageNode>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub selections: Vec<SelectionNode>,
}

#[derive(Debug, Deserialize)]
pub struct StageNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionNode {
    pub selection_type: Option<String>,
    pub entrant: Option<SelectionEntrant>,
    pub character: Option<CharacterNode>,
}

#[derive(Debug, Deserialize)]
pub struct SelectionEntrant {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct CharacterNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: Option<String>,
}

// ---------------------------------------------------------------------------
// PHASE_GROUP_SETS_QUERY_SLIM (identity pass)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlimPhaseGroupSetsData {
    pub phase_group: Option<SlimPhaseGroupNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlimPhaseGroupNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub sets: SetsPage<SlimSetNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlimSetNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub state: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub winner_id: Option<i64>,
    pub vod_url: Option<String>,
    pub completed_at: Option<i64>,
    pub full_round_text: Option<String>,
    pub round: Option<i64>,
    pub l_placement: Option<i64>,
    pub w_placement: Option<i64>,
    pub display_score: Option<String>,
    pub phase_group: Option<PhaseGroupInfo>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub slots: Vec<SlimSlotNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlimSlotNode {
    pub standing: Option<SlotStanding>,
    pub entrant: Option<SlimEntrantNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlimEntrantNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub initial_seed_num: Option<i64>,
    pub is_disqualified: Option<bool>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub participants: Vec<SlimParticipantNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlimParticipantNode {
    pub player: Option<PlayerNode>,
}

// ---------------------------------------------------------------------------
// PHASE_GROUP_GAMES_QUERY (games pass)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesPhaseGroupSetsData {
    pub phase_group: Option<GamesPhaseGroupNode>,
}

#[derive(Debug, Deserialize)]
pub struct GamesPhaseGroupNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub sets: SetsPage<GamesSetNode>,
}

#[derive(Debug, Deserialize)]
pub struct GamesSetNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub games: Vec<GameNode>,
}

// ---------------------------------------------------------------------------
// EVENT_STANDINGS_QUERY
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EventStandingsData {
    pub event: Option<EventStandingsNode>,
}

#[derive(Debug, Deserialize)]
pub struct EventStandingsNode {
    pub standings: StandingsPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingsPage {
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<StandingNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub placement: Option<i64>,
    pub is_final: Option<bool>,
    pub entrant: Option<StandingEntrant>,
}

#[derive(Debug, Deserialize)]
pub struct StandingEntrant {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
}
