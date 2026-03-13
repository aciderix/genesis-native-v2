// ── metabolism.rs ── Energy economy system ──────────────────────────────────
//
// Handles the complete energy lifecycle for every particle each tick:
//   • Day/night cycle with 500-tick period driving solar oscillation
//   • Base metabolic cost scaled by bonds, temperature, and epigenetic efficiency
//   • Beta photosynthesis from solar energy
//   • Catalyst chemosynthesis from hydrothermal vents & nutrient fields
//   • Type-specific maintenance costs (Data, Motor)
//   • Combo bonus energy injection
//   • Organism-level energy sharing across bonds
//   • Metabolite emission into scalar fields
//   • Death: particles below 0 energy die; eligible ones become deposits
//   • Deposit decay over time and from harvester proximity

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::{ParticleStore, SimRng};

use std::f32::consts::TAU;

/// Main metabolism system — runs every tick.
pub fn metabolism_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
    vents: &VentList,
    fields: &mut SimFields,
    day_night: &mut DayNightState,
    events: &mut EventLog,
    counters: &mut SimCounters,
    orgs: &OrganismRegistry,
) {
    let tick = counters.tick;
    let ws = config.world_size;
    let half = ws * 0.5;
    let ss = config.solar_strength;
    let sd = config.solar_dir;
    let base_temp = config.temperature;

    // ── Day / Night cycle (500-tick period) ─────────────────────────────
    let phase = (tick % 500) as f32 / 500.0;
    let solar_now = ss * (0.3 + 0.7 * (phase * TAU).sin().max(0.0));
    day_night.phase = phase;
    day_night.solar_now = solar_now;

    let n = store.alive_count;
    // We iterate by index over all slots.
    let len = store.x.len();

    // ── Per-particle metabolism ──────────────────────────────────────────
    for i in 0..len {
        if !store.alive[i] {
            // Handle deposit decay for dead-but-deposit particles
            if store.is_deposit[i] {
                deposit_decay(i, &mut store, &config, &mut fields, &mut events, tick);
            }
            continue;
        }

        // Particle state
        let px = store.x[i];
        let py = store.y[i];
        let pz = store.z[i];
        let ptype = store.ptype[i];
        let bond_count = store.bonds[i].len() as f32;
        let epi_eff = 0.8 + store.epi_weight[i] * 0.4; // epigenetic efficiency [0.8, 1.2]
        let cell_role = store.cell_role[i];

        // ── Spatial gradients ───────────────────────────────────────────
        // Temperature: warmer at bottom (y=0), cooler at top
        let temp_local = base_temp * (0.7 + 0.6 * (half - py) / ws);

        // Solar: varies with position facing solar_dir
        let dot_pos = px * sd.x + py * sd.y + pz * sd.z;
        let solar_local = solar_now * (0.6 + 0.4 * (dot_pos / half.max(1.0)).max(0.0));

        // ── Base metabolic cost ─────────────────────────────────────────
        // More bonds → slightly higher maintenance; temperature scales cost
        let base_cost = (0.02 + bond_count * 0.005) * (temp_local / base_temp.max(0.01)) / epi_eff;
        store.energy[i] -= base_cost;

        // ── Type-specific metabolism ────────────────────────────────────
        match ptype {
            // Beta: photosynthesis — gains solar energy
            ParticleType::Beta => {
                let mut gain = solar_local * 0.06 * epi_eff;
                // Digester role gets 1.5× photosynthesis
                if cell_role == CellRole::Digester {
                    gain *= 1.5;
                }
                store.energy[i] += gain;
            }

            // Catalyst: chemosynthesis from vents + nutrient absorption
            ParticleType::Catalyst => {
                // Vent proximity energy
                for vent in vents.0.iter() {
                    let dx = px - vent.position.x;
                    let dy = py - vent.position.y;
                    let dz = pz - vent.position.z;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    if dist < vent.radius {
                        let factor = 1.0 - dist / vent.radius;
                        store.energy[i] += vent.strength * factor * 0.08 * epi_eff;
                    }
                }

                // Nutrient field absorption
                let nut = fields.nutrient.sample(px, py, pz, ws);
                if nut > 0.1 {
                    let consume = (nut * 0.08).min(0.3);
                    store.energy[i] += consume * 2.0 * epi_eff;
                    fields.nutrient.inject(px, py, pz, ws, -consume);
                }
            }

            // Data: extra maintenance cost
            ParticleType::Data => {
                store.energy[i] -= 0.005;
            }

            // Motor: signal-driven energy cost
            ParticleType::Motor => {
                store.energy[i] -= store.signal[i].abs() * 0.02;
            }

            _ => {}
        }

        // ── Combo bonus energy ──────────────────────────────────────────
        store.energy[i] += store.combo_bonus[i] * 0.15;

        // ── Age increment ───────────────────────────────────────────────
        store.age[i] += 1;
    }

    // ── Organism energy sharing ─────────────────────────────────────────
    // Particles in the same organism share energy with bonded partners.
    // Energy flows from higher to lower at rate 0.12.
    // We collect deltas first to avoid order-dependent bias.
    let mut deltas = vec![0.0f32; len];

    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] || store.organism_id[i] < 0 {
            continue;
        }
        let my_energy = store.energy[i];
        // Iterate bonds
        let bonds: Vec<u32> = store.bonds[i].iter().copied().collect();
        for &bid in &bonds {
            if let Some(&j) = store.id_to_index.get(&bid) {
                if !store.alive[j] || store.is_deposit[j] {
                    continue;
                }
                let other_energy = store.energy[j];
                if my_energy > other_energy {
                    let flow = (my_energy - other_energy) * 0.12;
                    deltas[i] -= flow;
                    deltas[j] += flow;
                }
            }
        }
    }

    // Apply sharing deltas (halved because each pair is visited twice)
    for i in 0..len {
        store.energy[i] += deltas[i] * 0.5;
    }

    // ── Metabolite emission ─────────────────────────────────────────────
    // Every 5 ticks, organisms with 3+ particles emit to meta fields
    if tick % 5 == 0 {
        for (_oid, oinfo) in orgs.organisms.iter() {
            if oinfo.members.len() < 3 {
                continue;
            }
            // Compute centroid of organism
            let mut cx = 0.0f32;
            let mut cy = 0.0f32;
            let mut cz = 0.0f32;
            let mut count = 0u32;
            for &pid in &oinfo.members {
                if let Some(&idx) = store.id_to_index.get(&pid) {
                    if store.alive[idx] && !store.is_deposit[idx] {
                        cx += store.x[idx];
                        cy += store.y[idx];
                        cz += store.z[idx];
                        count += 1;
                    }
                }
            }
            if count == 0 {
                continue;
            }
            let inv = 1.0 / count as f32;
            cx *= inv;
            cy *= inv;
            cz *= inv;

            // Emit metabolites based on organism composition
            let strength = (count as f32 * 0.05).min(0.5);
            fields.meta_a.inject(cx, cy, cz, ws, strength);
            if count > 5 {
                fields.meta_b.inject(cx, cy, cz, ws, strength * 0.5);
            }
            if count > 10 {
                fields.meta_c.inject(cx, cy, cz, ws, strength * 0.3);
            }
        }
    }

    // ── Death & deposit creation ────────────────────────────────────────
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        if store.energy[i] < 0.0 {
            // Check if this particle is in an organism with enough energy to leave a deposit
            let org_id = store.organism_id[i];
            let should_deposit = if org_id >= 0 {
                if let Some(oinfo) = orgs.organisms.get(&(org_id as u32)) {
                    oinfo.energy > 2.0
                } else {
                    false
                }
            } else {
                false
            };

            if should_deposit {
                // Become a deposit instead of fully dying
                store.is_deposit[i] = true;
                store.energy[i] = 1.5; // seed deposit with some energy
                store.vx[i] = 0.0;
                store.vy[i] = 0.0;
                store.vz[i] = 0.0;
                // Clear bonds
                let my_id = store.id[i];
                let bond_list: Vec<u32> = store.bonds[i].iter().copied().collect();
                store.bonds[i].clear();
                for &bid in &bond_list {
                    if let Some(&j) = store.id_to_index.get(&bid) {
                        store.bonds[j].remove(&my_id);
                    }
                }
            } else {
                // Full death
                store.alive[i] = false;
                let my_id = store.id[i];
                let bond_list: Vec<u32> = store.bonds[i].iter().copied().collect();
                store.bonds[i].clear();
                for &bid in &bond_list {
                    if let Some(&j) = store.id_to_index.get(&bid) {
                        store.bonds[j].remove(&my_id);
                    }
                }
                // Release energy back to nutrient field
                let released = store.energy[i].abs().min(0.5);
                fields.nutrient.inject(
                    store.x[i], store.y[i], store.z[i], ws, released,
                );
            }
        }
    }
}

pub fn metabolism_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    vents: Res<VentList>,
    mut fields: ResMut<SimFields>,
    mut day_night: ResMut<DayNightState>,
    mut events: ResMut<EventLog>,
    mut counters: ResMut<SimCounters>,
    orgs: Res<OrganismRegistry>,
) {
    metabolism_inner(&mut *store, &*config, &*vents, &mut *fields, &mut *day_night, &mut *events, &mut *counters, &*orgs);
}

/// Handle deposit decay for a single particle index.
fn deposit_decay(
    i: usize,
    store: &mut ParticleStore,
    config: &SimConfig,
    fields: &mut SimFields,
    events: &mut EventLog,
    tick: u64,
) {
    // Deposits lose energy over time
    store.energy[i] -= 0.003;

    // Nearby catalysts (harvesters) drain deposits faster
    // We do a quick scan of nearby particles — this is O(n) but deposits are few
    let px = store.x[i];
    let py = store.y[i];
    let pz = store.z[i];
    let ws = config.world_size;
    let harvest_r = config.interaction_radius;
    let harvest_r2 = harvest_r * harvest_r;

    let len = store.x.len();
    for j in 0..len {
        if !store.alive[j] || store.is_deposit[j] || store.ptype[j] != ParticleType::Catalyst {
            continue;
        }
        let dx = store.x[j] - px;
        let dy = store.y[j] - py;
        let dz = store.z[j] - pz;
        let d2 = dx * dx + dy * dy + dz * dz;
        if d2 < harvest_r2 {
            // Harvester drains deposit
            store.energy[i] -= 0.002;
            // Harvester gains a small amount
            store.energy[j] += 0.001;
        }
    }

    // If deposit depleted, fully die
    if store.energy[i] <= 0.0 {
        store.alive[i] = false;
        store.is_deposit[i] = false;
        // Return a trace to nutrient field
        fields.nutrient.inject(px, py, pz, ws, 0.1);
    }
}
