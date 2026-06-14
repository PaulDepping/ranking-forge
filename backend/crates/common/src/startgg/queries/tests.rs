use super::*;

// ── SetNode helpers ───────────────────────────────────────────────────────────

fn make_slot(entrant_id: i64, score: Option<f64>) -> SetSlot {
    SetSlot {
        entrant: Some(SlotEntrant { id: entrant_id }),
        standing: score.map(|v| SlotStanding {
            stats: Some(SlotStats {
                score: Some(ScoreValue { value: Some(v) }),
            }),
        }),
    }
}

fn make_set(winner_id: Option<i64>, slots: Vec<SetSlot>) -> SetNode {
    SetNode {
        id: Some(1),
        winner_id,
        round: Some(1),
        full_round_text: None,
        total_games: Some(5),
        completed_at: None,
        vod_url: None,
        has_placeholder: None,
        state: None,
        identifier: None,
        phase_group: None,
        slots,
    }
}

#[test]
fn set_node_is_dq_false_for_normal_scores() {
    let set = make_set(
        Some(10),
        vec![make_slot(10, Some(3.0)), make_slot(20, Some(1.0))],
    );
    assert!(!set.is_dq());
}

#[test]
fn set_node_is_dq_true_when_any_score_negative() {
    let set = make_set(
        Some(10),
        vec![make_slot(10, Some(3.0)), make_slot(20, Some(-1.0))],
    );
    assert!(set.is_dq());
}

#[test]
fn set_node_is_dq_false_when_no_scores() {
    let set = make_set(
        Some(10),
        vec![
            SetSlot {
                entrant: Some(SlotEntrant { id: 10 }),
                standing: None,
            },
            SetSlot {
                entrant: Some(SlotEntrant { id: 20 }),
                standing: None,
            },
        ],
    );
    assert!(!set.is_dq());
}

#[test]
fn set_node_loser_id_returns_non_winner_entrant() {
    let set = make_set(
        Some(10),
        vec![make_slot(10, Some(3.0)), make_slot(20, Some(1.0))],
    );
    assert_eq!(set.loser_id(), Some(20));
}

#[test]
fn set_node_loser_id_none_when_no_winner_id() {
    let set = make_set(
        None,
        vec![make_slot(10, Some(3.0)), make_slot(20, Some(1.0))],
    );
    assert_eq!(set.loser_id(), None);
}

#[test]
fn set_node_loser_id_none_when_no_other_entrant() {
    let set = make_set(Some(10), vec![make_slot(10, Some(3.0))]);
    assert_eq!(set.loser_id(), None);
}

#[test]
fn set_node_scores_normal() {
    let set = make_set(
        Some(10),
        vec![make_slot(10, Some(3.0)), make_slot(20, Some(1.0))],
    );
    assert_eq!(set.scores(), (Some(3), Some(1)));
}

#[test]
fn set_node_scores_none_when_no_winner_id() {
    let set = make_set(
        None,
        vec![make_slot(10, Some(3.0)), make_slot(20, Some(1.0))],
    );
    assert_eq!(set.scores(), (None, None));
}

#[test]
fn set_node_scores_missing_when_no_standing() {
    let set = make_set(
        Some(10),
        vec![
            SetSlot {
                entrant: Some(SlotEntrant { id: 10 }),
                standing: None,
            },
            SetSlot {
                entrant: Some(SlotEntrant { id: 20 }),
                standing: None,
            },
        ],
    );
    assert_eq!(set.scores(), (None, None));
}

// ── EntrantNode helpers ───────────────────────────────────────────────────────

fn make_participant(tag: &str, user_id: Option<i64>) -> Participant {
    Participant {
        gamer_tag: tag.to_string(),
        user: user_id.map(|id| ParticipantUser { id }),
    }
}

fn make_entrant(participants: Vec<Participant>) -> EntrantNode {
    EntrantNode {
        id: 1_i64,
        initial_seed_num: Some(1),
        is_disqualified: Some(false),
        standing: None,
        participants,
    }
}

#[test]
fn entrant_node_display_name_from_first_participant() {
    let e = make_entrant(vec![
        make_participant("Mango", Some(1)),
        make_participant("Other", Some(2)),
    ]);
    assert_eq!(e.display_name(), "Mango");
}

#[test]
fn entrant_node_display_name_empty_when_no_participants() {
    let e = make_entrant(vec![]);
    assert_eq!(e.display_name(), "");
}

#[test]
fn entrant_node_startgg_user_id_from_first_participant() {
    let e = make_entrant(vec![make_participant("Mango", Some(12345))]);
    assert_eq!(e.startgg_user_id(), Some(12345));
}

#[test]
fn entrant_node_startgg_user_id_none_when_no_user() {
    let e = make_entrant(vec![make_participant("Guest", None)]);
    assert_eq!(e.startgg_user_id(), None);
}

#[test]
fn entrant_node_startgg_user_id_none_when_no_participants() {
    let e = make_entrant(vec![]);
    assert_eq!(e.startgg_user_id(), None);
}

#[test]
fn event_node_deserializes_videogame() {
    let json = r#"{
        "id": 2001,
        "name": "Melee Singles",
        "numEntrants": 8,
        "startAt": 1700040000,
        "slug": "tournament/test/event/melee-singles",
        "state": "COMPLETED",
        "isOnline": false,
        "type": 1,
        "teamRosterSize": null,
        "videogame": { "id": 1, "name": "Super Smash Bros. Melee" }
    }"#;
    let node: EventNode = serde_json::from_str(json).unwrap();
    let vg = node.videogame.unwrap();
    assert_eq!(vg.id, 1);
    assert_eq!(vg.name, "Super Smash Bros. Melee");
}

#[test]
fn event_node_videogame_is_none_when_absent() {
    let json = r#"{
        "id": 2001,
        "name": "Melee Singles",
        "numEntrants": 8,
        "startAt": null,
        "slug": null,
        "state": null,
        "isOnline": null,
        "type": null,
        "teamRosterSize": null
    }"#;
    let node: EventNode = serde_json::from_str(json).unwrap();
    assert!(node.videogame.is_none());
}
