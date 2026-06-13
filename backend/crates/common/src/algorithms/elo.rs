use std::collections::{HashMap, HashSet};

use serde_json::json;
use uuid::Uuid;

use super::{AlgorithmError, PlayerScore, RankingAlgorithm, ScoredSet};

pub struct Elo;

impl RankingAlgorithm for Elo {
    fn name(&self) -> &'static str {
        "elo"
    }

    fn compute(
        &self,
        config: &serde_json::Value,
        sets: &[ScoredSet],
    ) -> Result<Vec<PlayerScore>, AlgorithmError> {
        let k = config["k_factor"].as_f64().unwrap_or(32.0);
        let initial = config["initial_rating"].as_f64().unwrap_or(1500.0);

        let mut ratings: HashMap<Uuid, f64> = HashMap::new();

        for s in sets {
            let ra = *ratings.entry(s.winner_id).or_insert(initial);
            let rb = *ratings.entry(s.loser_id).or_insert(initial);
            let ea = 1.0 / (1.0 + 10.0_f64.powf((rb - ra) / 400.0));
            ratings.insert(s.winner_id, ra + k * (1.0 - ea));
            ratings.insert(s.loser_id, rb + k * (0.0 - (1.0 - ea)));
        }

        let player_ids: HashSet<Uuid> = sets
            .iter()
            .flat_map(|s| [s.winner_id, s.loser_id])
            .collect();

        let scores = player_ids
            .into_iter()
            .map(|pid| {
                let r = *ratings.get(&pid).unwrap_or(&initial);
                PlayerScore {
                    player_id: pid,
                    computed_rating: r,
                    display_data: json!({ "rating": r.round() as i64 }),
                    algorithm_state: json!({}),
                }
            })
            .collect();

        Ok(scores)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn set(winner: Uuid, loser: Uuid) -> ScoredSet {
        ScoredSet {
            winner_id: winner,
            loser_id: loser,
            completed_at: Utc::now(),
            winner_global_rating: None,
            loser_global_rating: None,
            is_external_winner: false,
            is_external_loser: false,
        }
    }

    #[test]
    fn elo_winner_gains_loser_loses() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let config = json!({"k_factor": 32.0});
        let scores = Elo.compute(&config, &[set(a, b)]).unwrap();

        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        let b_score = scores.iter().find(|s| s.player_id == b).unwrap();

        assert!(a_score.computed_rating > 1500.0);
        assert!(b_score.computed_rating < 1500.0);
        let delta = a_score.computed_rating - 1500.0;
        let loss = 1500.0 - b_score.computed_rating;
        assert!((delta - loss).abs() < 0.001);
    }

    #[test]
    fn elo_no_sets_returns_empty() {
        let scores = Elo.compute(&json!({}), &[]).unwrap();
        assert!(scores.is_empty());
    }

    #[test]
    fn elo_display_data_has_rating_key() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let scores = Elo.compute(&json!({}), &[set(a, b)]).unwrap();
        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        assert!(a_score.display_data["rating"].is_number());
    }
}
