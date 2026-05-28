//! Upset-factor calculation for double-elimination events.
//!
//! ## Algorithm
//!
//! 1. Convert each entrant's seed to a *projected losers-round number* using
//!    `seed_to_projected_round`. The table maps seed ranges to the losers round
//!    that seed is expected to reach based on standard DE bracket tiers
//!    (1st, 2nd, 3rd/4th, 5th, 7th, 9th, 13th, …).
//!
//! 2. Compute upset factor as:
//!
//!    ```text
//!    upset_factor = loser_projected_round - winner_projected_round
//!    ```
//!
//!    A **positive** value means the winner was seeded worse than expected — an upset.
//!    **Zero** means the seeds were equal. **Negative** means the favourite won.
//!
//! Sets where either entrant has a NULL seed (not linked to a known player) are
//! excluded from upset-factor calculations by the callers: `set_upset_factor` is
//! only invoked when both seeds are present, and the set is assigned UF = 0
//! otherwise.

/// Maps a seed to a projected losers-round number based on standard double-elimination
/// placement tiers: 1st, 2nd, 3rd/4th, 5th, 7th, 9th, 13th, 17th, 25th, 33rd, ...
///
/// Higher seed number = worse seed = lower projected round.
/// The tier boundaries follow the DE pattern where each losers round produces 2-way
/// or 4-way ties (5th, 7th, 9th, 13th, 17th, 25th, 33rd, 49th, ...).
/// Scale covers seeds up to ~12 288 (any realistic tournament size).
pub fn seed_to_projected_round(seed: i32) -> i32 {
    if seed <= 1 {
        26
    }
    // 1st
    else if seed <= 2 {
        25
    }
    // 2nd
    else if seed <= 4 {
        24
    }
    // 3rd/4th
    else if seed <= 6 {
        23
    }
    // 5th
    else if seed <= 8 {
        22
    }
    // 7th
    else if seed <= 12 {
        21
    }
    // 9th
    else if seed <= 16 {
        20
    }
    // 13th
    else if seed <= 24 {
        19
    }
    // 17th
    else if seed <= 32 {
        18
    }
    // 25th
    else if seed <= 48 {
        17
    }
    // 33rd
    else if seed <= 64 {
        16
    }
    // 49th
    else if seed <= 96 {
        15
    }
    // 65th
    else if seed <= 128 {
        14
    }
    // 97th
    else if seed <= 192 {
        13
    }
    // 129th
    else if seed <= 256 {
        12
    }
    // 193rd
    else if seed <= 384 {
        11
    }
    // 257th
    else if seed <= 512 {
        10
    }
    // 385th
    else if seed <= 768 {
        9
    }
    // 513th
    else if seed <= 1024 {
        8
    }
    // 769th
    else if seed <= 1536 {
        7
    }
    // 1025th
    else if seed <= 2048 {
        6
    }
    // 1537th
    else if seed <= 3072 {
        5
    }
    // 2049th
    else if seed <= 4096 {
        4
    }
    // 3073rd
    else if seed <= 6144 {
        3
    }
    // 4097th
    else if seed <= 8192 {
        2
    }
    // 6145th
    else if seed <= 12288 {
        1
    }
    // 8193rd
    else {
        0
    }
}

/// Upset factor for a single set. Positive = the winner exceeded expectations (upset);
/// negative = the winner was favoured (expected result). `NULL` seeds are treated as
/// the largest possible seed (round = 0).
pub fn set_upset_factor(winner_seed: i32, loser_seed: i32) -> i32 {
    let wr = seed_to_projected_round(winner_seed);
    let lr = seed_to_projected_round(loser_seed);
    lr - wr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_1_beats_seed_100_is_expected() {
        assert!(set_upset_factor(1, 100) < 0);
    }

    #[test]
    fn seed_100_beats_seed_1_is_upset() {
        assert!(set_upset_factor(100, 1) > 0);
    }

    #[test]
    fn equal_seeds_zero_uf() {
        assert_eq!(set_upset_factor(5, 5), 0);
    }

    #[test]
    fn tiers_are_monotone_decreasing() {
        let seeds = [
            1, 2, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536,
            2048, 3072, 4096, 6144, 8192, 12288, 12289,
        ];
        let mut prev = seed_to_projected_round(seeds[0]);
        for &s in &seeds[1..] {
            let r = seed_to_projected_round(s);
            assert!(r <= prev, "seed {s}: round {r} should be ≤ {prev}");
            prev = r;
        }
    }
}
