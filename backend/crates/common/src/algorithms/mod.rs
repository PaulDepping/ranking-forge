pub mod elo;
pub mod glicko2;

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ScoredSet {
    pub winner_id: Uuid,
    pub loser_id: Uuid,
    pub completed_at: DateTime<Utc>,
    pub winner_global_rating: Option<f64>,
    pub loser_global_rating: Option<f64>,
    pub is_external_winner: bool,
    pub is_external_loser: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerScore {
    pub player_id: Uuid,
    pub computed_rating: f64,
    pub display_data: serde_json::Value,
    pub algorithm_state: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum AlgorithmError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("computation failed: {0}")]
    ComputationFailed(String),
}

pub trait RankingAlgorithm: Send + Sync {
    fn name(&self) -> &'static str;
    fn compute(
        &self,
        config: &serde_json::Value,
        sets: &[ScoredSet],
    ) -> Result<Vec<PlayerScore>, AlgorithmError>;
}

pub struct AlgorithmRegistry {
    algorithms: HashMap<&'static str, Box<dyn RankingAlgorithm>>,
}

impl AlgorithmRegistry {
    pub fn new() -> Self {
        let mut r = Self {
            algorithms: HashMap::new(),
        };
        r.register(Box::new(elo::Elo));
        r.register(Box::new(glicko2::Glicko2));
        r
    }

    fn register(&mut self, algo: Box<dyn RankingAlgorithm>) {
        self.algorithms.insert(algo.name(), algo);
    }

    pub fn get(&self, name: &str) -> Option<&dyn RankingAlgorithm> {
        self.algorithms.get(name).map(|b| b.as_ref())
    }
}

impl Default for AlgorithmRegistry {
    fn default() -> Self {
        Self::new()
    }
}
