use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use serde_json::json;
use uuid::Uuid;

use super::{AlgorithmError, PlayerScore, RankingAlgorithm, ScoredSet};

pub struct Glicko2;

const SCALE: f64 = 173.7178;

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi * phi / (PI * PI)).sqrt()
}

fn e(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
    1.0 / (1.0 + (-g(phi_j) * (mu - mu_j)).exp())
}

fn new_sigma(sigma: f64, phi: f64, v: f64, delta: f64, tau: f64) -> f64 {
    let a = (sigma * sigma).ln();
    let delta_sq = delta * delta;
    let phi_sq = phi * phi;

    let f = |x: f64| -> f64 {
        let ex = x.exp();
        let num = ex * (delta_sq - phi_sq - v - ex);
        let den = 2.0 * (phi_sq + v + ex).powi(2);
        num / den - (x - a) / (tau * tau)
    };

    let mut a_val = a;
    let mut b_val = if delta_sq > phi_sq + v {
        (delta_sq - phi_sq - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * tau) < 0.0 {
            k += 1.0;
        }
        a - k * tau
    };

    let eps = 1e-6;
    let mut fa = f(a_val);
    let mut fb = f(b_val);

    while (b_val - a_val).abs() > eps {
        let c = a_val + (a_val - b_val) * fa / (fb - fa);
        let fc = f(c);
        if fc * fb < 0.0 {
            a_val = b_val;
            fa = fb;
        } else {
            fa /= 2.0;
        }
        b_val = c;
        fb = fc;
    }

    ((a_val + b_val) / 2.0 / 2.0).exp().sqrt()
}

impl RankingAlgorithm for Glicko2 {
    fn name(&self) -> &'static str {
        "glicko2"
    }

    fn compute(
        &self,
        config: &serde_json::Value,
        sets: &[ScoredSet],
    ) -> Result<Vec<PlayerScore>, AlgorithmError> {
        let tau = config["tau"].as_f64().unwrap_or(0.5);
        let initial_rd = config["initial_rd"].as_f64().unwrap_or(350.0);
        let initial_vol = config["initial_volatility"].as_f64().unwrap_or(0.06);

        let initial_mu = 0.0_f64;
        let initial_phi = initial_rd / SCALE;

        let mut state: HashMap<Uuid, (f64, f64, f64)> = HashMap::new();

        for s in sets {
            let (mu_w, phi_w, sigma_w) = *state
                .entry(s.winner_id)
                .or_insert((initial_mu, initial_phi, initial_vol));
            let (mu_l, phi_l, sigma_l) = *state
                .entry(s.loser_id)
                .or_insert((initial_mu, initial_phi, initial_vol));

            let g_l = g(phi_l);
            let e_w = e(mu_w, mu_l, phi_l);
            let v_w = 1.0 / (g_l * g_l * e_w * (1.0 - e_w));
            let delta_w = v_w * g_l * (1.0 - e_w);
            let sigma_w_new = new_sigma(sigma_w, phi_w, v_w, delta_w, tau);
            let phi_star_w = (phi_w * phi_w + sigma_w_new * sigma_w_new).sqrt();
            let phi_w_new = 1.0 / (1.0 / (phi_star_w * phi_star_w) + 1.0 / v_w).sqrt();
            let mu_w_new = mu_w + phi_w_new * phi_w_new * g_l * (1.0 - e_w);

            let g_w = g(phi_w);
            let e_l = e(mu_l, mu_w, phi_w);
            let v_l = 1.0 / (g_w * g_w * e_l * (1.0 - e_l));
            let delta_l = v_l * g_w * (0.0 - e_l);
            let sigma_l_new = new_sigma(sigma_l, phi_l, v_l, delta_l, tau);
            let phi_star_l = (phi_l * phi_l + sigma_l_new * sigma_l_new).sqrt();
            let phi_l_new = 1.0 / (1.0 / (phi_star_l * phi_star_l) + 1.0 / v_l).sqrt();
            let mu_l_new = mu_l + phi_l_new * phi_l_new * g_w * (0.0 - e_l);

            state.insert(s.winner_id, (mu_w_new, phi_w_new, sigma_w_new));
            state.insert(s.loser_id, (mu_l_new, phi_l_new, sigma_l_new));
        }

        let player_ids: HashSet<Uuid> = sets
            .iter()
            .flat_map(|s| [s.winner_id, s.loser_id])
            .collect();

        let scores = player_ids
            .into_iter()
            .map(|pid| {
                let (mu, phi, sigma) = *state
                    .get(&pid)
                    .unwrap_or(&(initial_mu, initial_phi, initial_vol));
                let r = SCALE * mu + 1500.0;
                let rd = (SCALE * phi).round() as i64;
                PlayerScore {
                    player_id: pid,
                    computed_rating: r,
                    display_data: json!({ "rating": r.round() as i64, "rd": rd }),
                    algorithm_state: json!({ "mu": mu, "phi": phi, "sigma": sigma }),
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
    fn glicko2_winner_higher_than_loser() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let config = json!({"tau": 0.5, "initial_rd": 350.0, "initial_volatility": 0.06});
        let scores = Glicko2.compute(&config, &[set(a, b)]).unwrap();
        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        let b_score = scores.iter().find(|s| s.player_id == b).unwrap();
        assert!(a_score.computed_rating > b_score.computed_rating);
    }

    #[test]
    fn glicko2_display_data_has_rating_and_rd() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let config = json!({"tau": 0.5, "initial_rd": 350.0, "initial_volatility": 0.06});
        let scores = Glicko2.compute(&config, &[set(a, b)]).unwrap();
        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        assert!(a_score.display_data["rating"].is_number());
        assert!(a_score.display_data["rd"].is_number());
    }
}
