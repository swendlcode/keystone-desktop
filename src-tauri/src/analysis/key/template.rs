//! The template voter: cosine matching of the PCP against the profile set.

use crate::analysis::key::profiles::{ProfileId, ProfileSet, AMODAL_PENALTY};
use crate::analysis::key::skey::{key_to_label, KeyProbabilities};
use crate::analysis::spectral::chroma::Pcp;
use crate::error::Result;

/// How many standard deviations of separation between the 24 candidates count
/// as a decisive match.
///
/// The softmax is taken over **standardised** similarities, not raw ones, and
/// that is the load-bearing detail. Cosine similarities between two
/// non-negative vectors all sit in a narrow high band — on real chroma every
/// one of the 24 candidates scores between about 0.84 and 0.91 — so a softmax
/// with an absolute temperature over those numbers is near-uniform however
/// decisive the match was. Standardising first makes the temperature mean
/// something scale-free, and makes a flat chroma (where every candidate scores
/// the same) come out uniform, which is the honest answer for it.
///
/// Provisional, like the fusion weights: ENGINE.md 7.1 fits this on the eval
/// corpus rather than arguing about it.
pub const SOFTMAX_TEMPERATURE: f32 = 0.30;

#[derive(Debug, Clone)]
pub struct TemplateVote {
    /// Converted to the same 24-label space as the model voter so fusion is a
    /// vector sum rather than a special case per voter.
    pub probabilities: KeyProbabilities,
    pub best_profile: ProfileId,
    /// Distance to the winning rotation, kept for the eval harness.
    pub best_distance: f32,
    /// Standard deviation of the 24 similarities.
    ///
    /// This, not the top similarity, is what says whether the chroma
    /// distinguishes between keys at all. Percussion and noise score ~0.9
    /// against *every* candidate, so the absolute number is meaningless while
    /// the spread collapses — measured at 0.02 on synthetic percussion against
    /// 0.17 on synthetic tonal material. It is the input to the confidence
    /// term, and it is why atonal material comes back ambiguous rather than
    /// guessed.
    pub spread: f32,
}

/// The distribution to report when there is nothing to go on: flat, so no key
/// can be read out of it and the confidence term sees zero spread.
fn uninformative(profile: ProfileId) -> TemplateVote {
    TemplateVote {
        probabilities: KeyProbabilities { p: [1.0 / 24.0; 24] },
        best_profile: profile,
        best_distance: 1.0,
        spread: 0.0,
    }
}

/// Cosine distance, not correlation: Sha'ath found cosine consistently better
/// on dance music, and the reason is that a whitened, gated HPCP has most bins
/// at zero, where correlation's mean-centring works against you — it treats an
/// absent pitch class as evidence *for* a key whose profile is also low there.
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..n {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    if !(norm_a > 0.0) || !(norm_b > 0.0) {
        return 1.0;
    }
    1.0 - (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(-1.0, 1.0)
}

/// Folds the 36-bin PCP to 12 and matches it.
pub fn match_pcp(pcp: &Pcp, profiles: &ProfileSet) -> Result<TemplateVote> {
    Ok(match_chroma(&pcp.fold_to_12(), profiles))
}

/// Matches all 12 rotations of every profile and reduces them to one
/// distribution over the 24 keys.
///
/// A minor label can be reached by three profiles (flat third, natural third,
/// amodal); the best of them is the label's score. The amodal win is **reported
/// as minor** and carries [`AMODAL_PENALTY`], for the reason recorded with that
/// constant.
pub fn match_chroma(chroma: &[f32; 12], profiles: &ProfileSet) -> TemplateVote {
    if !(chroma.iter().sum::<f32>() > 0.0) || profiles.profiles.is_empty() {
        return uninformative(ProfileId::Amodal);
    }
    let (scores, winner) = score_rotations(chroma, profiles);
    let mean = scores.iter().sum::<f32>() / 24.0;
    let spread = (scores.iter().map(|s| (s - mean) * (s - mean)).sum::<f32>() / 24.0).sqrt();
    let top = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !(spread > 1e-6) {
        let mut flat = uninformative(winner);
        flat.best_distance = 1.0 - top;
        return flat;
    }
    TemplateVote {
        probabilities: KeyProbabilities { p: standardised_softmax(&scores, mean, spread, top) },
        best_profile: winner,
        best_distance: 1.0 - top,
        spread,
    }
}

/// The best similarity reached for each of the 24 labels, and which profile won
/// outright.
fn score_rotations(chroma: &[f32; 12], profiles: &ProfileSet) -> ([f32; 24], ProfileId) {
    let mut best = [f32::NEG_INFINITY; 24];
    let mut winner = (f32::NEG_INFINITY, ProfileId::Major);
    let mut rotated = [0.0f32; 12];
    for profile in &profiles.profiles {
        let scale = if profile.id == ProfileId::Amodal { AMODAL_PENALTY } else { 1.0 };
        let mode = profile.id.mode();
        for root in 0..12usize {
            for (i, slot) in rotated.iter_mut().enumerate() {
                *slot = profile.weights[(i + 12 - root) % 12];
            }
            let similarity = (1.0 - cosine_distance(chroma, &rotated)) * scale;
            let label = key_to_label(root as u8, mode);
            if similarity > best[label] {
                best[label] = similarity;
            }
            if similarity > winner.0 {
                winner = (similarity, profile.id);
            }
        }
    }
    // A label no profile reached would poison the mean. With one major and at
    // least one minor profile this cannot happen, but a caller is free to pass
    // a set that does not cover both modes.
    let floor = best.iter().copied().filter(|s| s.is_finite()).fold(f32::INFINITY, f32::min);
    let floor = if floor.is_finite() { floor } else { 0.0 };
    for s in best.iter_mut() {
        if !s.is_finite() {
            *s = floor;
        }
    }
    (best, winner.1)
}

fn standardised_softmax(scores: &[f32; 24], mean: f32, spread: f32, top: f32) -> [f32; 24] {
    let mut p = [0.0f32; 24];
    let mut total = 0.0f32;
    let peak_z = (top - mean) / spread;
    for (slot, s) in p.iter_mut().zip(scores.iter()) {
        let z = (s - mean) / spread;
        *slot = ((z - peak_z) / SOFTMAX_TEMPERATURE).exp();
        total += *slot;
    }
    if total > 0.0 {
        for slot in p.iter_mut() {
            *slot /= total;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::key::profiles::shipping_set;
    use crate::analysis::key::skey::label_to_key;
    use crate::models::KeyMode;

    fn triad(root: usize, intervals: &[usize]) -> [f32; 12] {
        let mut c = [0.0f32; 12];
        for i in intervals {
            c[(root + i) % 12] += 1.0;
        }
        c
    }

    #[test]
    fn cosine_distance_is_zero_for_parallel_and_one_for_orthogonal() {
        assert!(cosine_distance(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).abs() < 1e-6);
        assert!((cosine_distance(&[1.0, 0.0], &[0.0, 1.0]) - 1.0).abs() < 1e-6);
        // A zero vector has no direction; reporting maximum distance is what
        // keeps a silent frame from matching every key equally well.
        assert_eq!(cosine_distance(&[0.0, 0.0], &[1.0, 1.0]), 1.0);
    }

    #[test]
    fn a_flat_chroma_gives_a_uniform_distribution_and_no_spread() {
        let vote = match_chroma(&[1.0 / 12.0; 12], &shipping_set());
        assert!(vote.spread < 0.08, "flat chroma spread {}", vote.spread);
        for p in vote.probabilities.p {
            assert!((p - 1.0 / 24.0).abs() < 0.05, "not near uniform: {p}");
        }
    }

    #[test]
    fn an_empty_chroma_does_not_produce_a_key() {
        let vote = match_chroma(&[0.0; 12], &shipping_set());
        assert_eq!(vote.spread, 0.0);
        assert_eq!(vote.probabilities.p[0], 1.0 / 24.0);
    }

    #[test]
    fn a_single_pitch_class_matches_amodal_and_reports_minor() {
        let mut chroma = [0.0f32; 12];
        chroma[9] = 1.0;
        let vote = match_chroma(&chroma, &shipping_set());
        assert_eq!(vote.best_profile, ProfileId::Amodal);
        let (best, _) = vote.probabilities.best();
        assert_eq!(label_to_key(best), (9, KeyMode::Minor));
    }

    #[test]
    fn a_bare_triad_finds_its_own_root_in_every_key() {
        for root in 0..12usize {
            let vote = match_chroma(&triad(root, &[0, 4, 7]), &shipping_set());
            let (best, _) = vote.probabilities.best();
            let (r, m) = label_to_key(best);
            assert_eq!(
                (r as usize, m),
                (root, KeyMode::Major),
                "major triad on {root} read as {r} {m:?}"
            );
            let vote = match_chroma(&triad(root, &[0, 3, 7]), &shipping_set());
            let (best, _) = vote.probabilities.best();
            let (r, m) = label_to_key(best);
            assert_eq!(
                (r as usize, m),
                (root, KeyMode::Minor),
                "minor triad on {root} read as {r} {m:?}"
            );
        }
    }

    #[test]
    fn the_distribution_is_a_distribution() {
        let vote = match_chroma(&triad(0, &[0, 4, 7]), &shipping_set());
        let total: f32 = vote.probabilities.p.iter().sum();
        assert!((total - 1.0).abs() < 1e-4, "probabilities sum to {total}");
        assert!(vote.probabilities.p.iter().all(|p| *p >= 0.0));
    }
}
