//! Extended metrics computation for Genesis 2.0
//! 
//! Provides Assembly Index, Causal Emergence, Transfer Entropy,
//! Innovation Rate, and Phylogenetic Diversity metrics.

pub use genesis_core::metrics::*;

/// Compute Transfer Entropy approximation between two time series
pub fn transfer_entropy(source: &[f32], target: &[f32], lag: usize) -> f32 {
    if source.len() < lag + 2 || target.len() < lag + 2 { return 0.0; }
    // Simplified TE: correlation of source[t-lag] with target[t] - target[t-1]
    let n = source.len() - lag;
    let mut sum = 0.0f32;
    for t in lag..source.len().min(target.len()) {
        let delta_target = if t > 0 { target[t] - target[t-1] } else { 0.0 };
        sum += source[t - lag] * delta_target;
    }
    (sum / n as f32).abs()
}

/// Compute Causal Emergence: difference between macro and micro-level entropy
pub fn causal_emergence(group_sizes: &[usize], total_population: usize) -> f32 {
    if total_population == 0 || group_sizes.is_empty() { return 0.0; }
    // H_macro - H_micro approximation
    let n = total_population as f32;
    let mut h_macro = 0.0f32;
    for &size in group_sizes {
        let p = size as f32 / n;
        if p > 0.0 {
            h_macro -= p * p.ln();
        }
    }
    let h_micro = (n).ln(); // Maximum entropy if all independent
    (h_macro / h_micro.max(0.001)).clamp(0.0, 1.0)
}
