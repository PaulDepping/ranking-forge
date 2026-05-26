use super::{
    CURRENT_USER_QUERY, EVENT_ENTRANTS_QUERY, EVENT_PHASES_QUERY, EVENT_SETS_QUERY,
    GAME_SEARCH_QUERY, TOURNAMENT_ALL_EVENTS_QUERY, TOURNAMENT_ENTRANT_LIST_QUERY,
    TOURNAMENT_EVENTS_QUERY, TOURNAMENT_PARTICIPANTS_QUERY, TOURNAMENTS_BY_USER_ALL_GAMES_QUERY,
    TOURNAMENTS_BY_USER_QUERY, USER_BY_SLUG_QUERY,
};

fn assert_query_parses(query: &'static str) {
    graphql_parser::parse_query::<String>(query)
        .unwrap_or_else(|e| panic!("query failed to parse: {e}"));
}

// Validates the schema file itself is well-formed SDL — catches corruption or
// truncation of schema.graphql.
#[test]
fn schema_parses() {
    graphql_parser::parse_schema::<String>(include_str!("../schema.graphql"))
        .unwrap_or_else(|e| panic!("schema.graphql failed to parse: {e}"));
}

// Note: graphql-parser validates syntax only, not field names against the
// schema. A field-name typo (e.g. `displayNme`) would not be caught here.
// These tests catch: garbled syntax, mismatched braces, invalid variable
// declarations, and unknown query keywords. Full schema-aware validation
// would require apollo-compiler or similar — that is left as future work.
//
// Tests cover all 12 query constants used in StartggClient operations.

#[test]
fn game_search_query_is_valid() {
    assert_query_parses(GAME_SEARCH_QUERY);
}

#[test]
fn user_by_slug_query_is_valid() {
    assert_query_parses(USER_BY_SLUG_QUERY);
}

#[test]
fn tournaments_by_user_query_is_valid() {
    assert_query_parses(TOURNAMENTS_BY_USER_QUERY);
}

#[test]
fn tournaments_by_user_all_games_query_is_valid() {
    assert_query_parses(TOURNAMENTS_BY_USER_ALL_GAMES_QUERY);
}

#[test]
fn event_entrants_query_is_valid() {
    assert_query_parses(EVENT_ENTRANTS_QUERY);
}

#[test]
fn event_sets_query_is_valid() {
    assert_query_parses(EVENT_SETS_QUERY);
}

#[test]
fn event_phases_query_is_valid() {
    assert_query_parses(EVENT_PHASES_QUERY);
}

#[test]
fn tournament_events_query_is_valid() {
    assert_query_parses(TOURNAMENT_EVENTS_QUERY);
}

#[test]
fn tournament_entrant_list_query_is_valid() {
    assert_query_parses(TOURNAMENT_ENTRANT_LIST_QUERY);
}

#[test]
fn tournament_participants_query_is_valid() {
    assert_query_parses(TOURNAMENT_PARTICIPANTS_QUERY);
}

#[test]
fn tournament_all_events_query_is_valid() {
    assert_query_parses(TOURNAMENT_ALL_EVENTS_QUERY);
}

#[test]
fn current_user_query_is_valid() {
    assert_query_parses(CURRENT_USER_QUERY);
}
