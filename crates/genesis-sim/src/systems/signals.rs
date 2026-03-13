// ── signals.rs ── Signal propagation & phase oscillation ────────────────────
//
// Two systems:
//   1. propagate_signals_system — iterative neural-network-style signal relay
//      across particle bonds using conductance matrices.
//   2. update_phase_system — Kuramoto-model phase oscillator coupling.

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::ParticleStore;

use std::f32::consts::TAU;

/// Propagate signals through bonded particle networks.
///
/// Runs 8 iterations per tick for correct neural-network-style propagation.
/// Each alive, non-deposit particle with bonds:
///   - Averages incoming signals from bonded partners weighted by sig_cond matrix
///   - Adds memory bias (memory * 0.1)
///   - Data-type particles scale signal by (0.5 + memory)
///   - Sensor role amplifies signal by 1.4×
///   - Combo bonus modifier: (1 + combo_bonus * 0.5)
///   - Exponential moving average: new = old*0.3 + input*0.7
///   - Clamp to [-3, 3]
///
/// After the 8 iterations, colony bond averaging blends signals with 0.4 weight.
/// Memory update: memory += (signal - memory) * 0.03, clamp [-2, 2].
pub fn propagate_signals_inner(
    store: &mut ParticleStore,
    matrices: &SimMatrices,
    colonies: &ColonyRegistry,
) {
    let len = store.x.len();
    if len == 0 {
        return;
    }

    // ── 8 iterations of signal propagation ──────────────────────────────
    for _iter in 0..8 {
        // Collect new signals into a buffer to avoid read-write aliasing
        let mut new_signals = vec![f32::NAN; len]; // NAN = "no update"

        for i in 0..len {
            if !store.alive[i] || store.is_deposit[i] || store.bonds[i].is_empty() {
                continue;
            }

            let ptype_i = store.ptype[i] as usize;
            let cell_role_i = store.cell_role[i];
            let memory_i = store.memory[i];
            let combo_i = store.combo_bonus[i];

            // Weighted average of incoming signals from bonded particles
            let mut sum = 0.0f32;
            let mut weight_sum = 0.0f32;
            let bonds_list: Vec<u32> = store.bonds[i].iter().copied().collect();

            for &bid in &bonds_list {
                if let Some(&j) = store.id_to_index.get(&bid) {
                    if !store.alive[j] {
                        continue;
                    }
                    let ptype_j = store.ptype[j] as usize;
                    // Conductance weight from matrix
                    let w = matrices.sig_cond[ptype_i][ptype_j].max(0.01);
                    sum += store.signal[j] * w;
                    weight_sum += w;
                }
            }

            if weight_sum < 0.001 {
                continue;
            }

            let mut input = sum / weight_sum;

            // Add memory bias
            input += memory_i * 0.1;

            // Data type: signal modulated by memory
            if ptype_i == 3 {
                input *= 0.5 + memory_i;
            }

            // Combo bonus modifier
            input *= 1.0 + combo_i * 0.5;

            // Sensor role amplification
            if cell_role_i == CellRole::Sensor {
                input *= 1.4;
            }

            // Exponential moving average with current signal
            let current = store.signal[i];
            let blended = current * 0.3 + input * 0.7;

            // Clamp to [-3, 3]
            new_signals[i] = blended.clamp(-3.0, 3.0);
        }

        // Apply buffered signals
        for i in 0..len {
            if !new_signals[i].is_nan() {
                store.signal[i] = new_signals[i];
            }
        }
    }

    // ── Colony bond signal averaging ────────────────────────────────────
    // For each colony, average signals between bonded organisms with 0.4 blend.
    for (_cid, colony) in colonies.colonies.iter() {
        for &(a_pid, b_pid) in &colony.bonds {
            let a_idx = store.id_to_index.get(&a_pid).copied();
            let b_idx = store.id_to_index.get(&b_pid).copied();
            if let (Some(ai), Some(bi)) = (a_idx, b_idx) {
                if store.alive[ai] && store.alive[bi] {
                    let sa = store.signal[ai];
                    let sb = store.signal[bi];
                    let avg = (sa + sb) * 0.5;
                    // Blend toward average with weight 0.4
                    store.signal[ai] = sa + (avg - sa) * 0.4;
                    store.signal[bi] = sb + (avg - sb) * 0.4;
                }
            }
        }
    }

    // ── Memory update ───────────────────────────────────────────────────
    // Memory drifts slowly toward the current signal value.
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }
        let sig = store.signal[i];
        let mem = store.memory[i];
        store.memory[i] = (mem + (sig - mem) * 0.03).clamp(-2.0, 2.0);
    }
}

pub fn propagate_signals_system(
    mut store: ResMut<ParticleStore>,
    matrices: Res<SimMatrices>,
    colonies: Res<ColonyRegistry>,
) {
    propagate_signals_inner(&mut *store, &*matrices, &*colonies);
}

/// Update particle phases using a Kuramoto-model oscillator.
///
/// Each alive particle's phase advances at a base rate plus type- and
/// energy-dependent offsets, then is coupled to bonded neighbors via
/// the sig_cond matrix:
///
///   phase += 0.05 + ptype_idx * 0.01 + energy * 0.005
///   for each bonded particle j:
///     phase += 0.03 * sig_cond[type_i][type_j] * sin(phase_j - phase_i)
///   wrap to [0, TAU)
pub fn update_phase_inner(
    store: &mut ParticleStore,
    matrices: &SimMatrices,
) {
    let len = store.x.len();
    if len == 0 {
        return;
    }

    // Collect phase deltas into a buffer (coupling depends on current phases)
    let mut phase_deltas = vec![0.0f32; len];

    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        let ptype_i = store.ptype[i] as usize;
        let energy_i = store.energy[i];
        let phase_i = store.phase[i];

        // Base phase advance
        let mut delta = 0.05 + ptype_i as f32 * 0.01 + energy_i * 0.005;

        // Kuramoto coupling with bonded particles
        let bonds_list: Vec<u32> = store.bonds[i].iter().copied().collect();
        for &bid in &bonds_list {
            if let Some(&j) = store.id_to_index.get(&bid) {
                if !store.alive[j] {
                    continue;
                }
                let ptype_j = store.ptype[j] as usize;
                let phase_j = store.phase[j];
                let coupling = matrices.sig_cond[ptype_i][ptype_j];
                delta += 0.03 * coupling * (phase_j - phase_i).sin();
            }
        }

        phase_deltas[i] = delta;
    }

    // Apply phase updates and wrap to [0, TAU)
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }
        store.phase[i] = (store.phase[i] + phase_deltas[i]).rem_euclid(TAU);
    }
}

pub fn update_phase_system(
    mut store: ResMut<ParticleStore>,
    matrices: Res<SimMatrices>,
) {
    update_phase_inner(&mut *store, &*matrices);
}
