//! Force computation system — the simulation's physics core.
//!
//! Ports the TypeScript `applyForces()` method to Rust. This is the most
//! performance-critical system and runs every tick for every alive particle.
//!
//! ## Force Types (17+)
//!
//! 1.  **Bond spring**: Hooke's law toward `bond_distance`
//! 2.  **Affinity**: Matrix-driven attraction/repulsion for non-bonded pairs
//! 3.  **Hard repulsion**: Prevents particle overlap (below `bond_distance * 0.5`)
//! 4.  **Motor predation push**: Motors push into foreign organisms
//! 5.  **Defense repulsion**: Defense-role particles repel foreign particles
//! 6.  **Immune repulsion**: Different immune signatures cause repulsion
//! 7.  **Solar radiation**: Beta particles pushed in solar direction
//! 8.  **Digester solar boost**: Digester cells get extra solar energy capture
//! 9.  **Vent attraction**: Catalyst particles attracted to thermal vents
//! 10. **Nutrient gradient**: Catalysts in organisms follow nutrient gradients
//! 11. **Pheromone attraction**: Catalysts follow attractive pheromone gradients
//! 12. **Alarm pheromone avoidance**: Particles flee from alarm pheromone
//! 13. **Wave field response**: Catalyst/Motor react to wave amplitude field
//! 14. **Metabolite gradient**: All particles follow metabolite gradients
//! 15. **Symbol field response**: Organisms with Catalyst follow symbol fields
//! 16. **Motor output**: Signal-driven force, phase-modulated, motor cell boost
//! 17. **Epigenetic flight**: High epi_weight particles flee danger
//! 18. **Brownian motion**: Random thermal jitter
//!
//! ## Algorithm
//!
//! Uses a two-pass approach to avoid borrow conflicts:
//! - **Pass 1**: Compute force accumulator `(fx, fy, fz)` for every alive particle
//! - **Pass 2**: Add accumulated forces to particle velocities

use bevy::prelude::*;
use crate::particle_store::{ParticleStore, SimRng};
use crate::config::SimConfig;
use crate::resources::*;
use crate::util::SpatialGrid;
use crate::components::{ParticleType, CellRole, NUM_TYPES};

// ---------------------------------------------------------------------------
// Constants — matching the TypeScript reference implementation
// ---------------------------------------------------------------------------

/// Base strength of the bond spring force (Hooke's law spring constant).
const BOND_SPRING_K: f32 = 0.3;

/// Strength multiplier for matrix-driven affinity forces.
const AFFINITY_STRENGTH: f32 = 0.04;

/// Strength of the hard repulsion force (prevents particle overlap).
const REPULSION_STRENGTH: f32 = 0.5;

/// Motor particles push into foreign organisms with this force.
const MOTOR_PREDATION_PUSH: f32 = 0.05;

/// Defense-role particles repel foreign particles with this force.
const DEFENSE_REPULSION: f32 = 0.03;

/// Immune signature mismatch repulsion strength.
const IMMUNE_REPULSION: f32 = 0.04;

/// Solar force is scaled by `config.solar_strength * SOLAR_FORCE_SCALE`.
const SOLAR_FORCE_SCALE: f32 = 0.1;

/// Extra solar absorption for Digester-role cells.
const DIGESTER_SOLAR_BONUS: f32 = 0.05;

/// How strongly Catalyst particles are attracted to thermal vents.
const VENT_ATTRACTION: f32 = 0.02;

/// Strength of nutrient gradient following for Catalysts in organisms.
const NUTRIENT_GRADIENT_STR: f32 = 0.03;

/// Strength of attractive pheromone gradient following.
const PHERO_ATTR_STRENGTH: f32 = 0.025;

/// Strength of alarm pheromone avoidance (repulsion from gradient).
const PHERO_ALARM_STRENGTH: f32 = 0.03;

/// Wave field response strength for Catalyst/Motor particles.
const WAVE_RESPONSE_STR: f32 = 0.02;

/// Metabolite gradient response strength.
const METABOLITE_STR: f32 = 0.015;

/// Symbol field response strength for organisms with Catalyst.
const SYMBOL_FIELD_STR: f32 = 0.02;

/// Motor output: signal-to-force conversion factor.
const MOTOR_SIGNAL_FORCE: f32 = 0.08;

/// Boost factor for motor cell role.
const MOTOR_CELL_BOOST: f32 = 1.5;

/// Epigenetic flight response strength.
const EPI_FLIGHT_STR: f32 = 0.04;

/// Brownian motion scale = `config.temperature * BROWNIAN_SCALE`.
const BROWNIAN_SCALE: f32 = 0.02;

/// Radius around vents for attraction effect.
const VENT_EFFECT_RADIUS: f32 = 12.0;

// ---------------------------------------------------------------------------
// Force system
// ---------------------------------------------------------------------------

/// Compute and apply all forces to particle velocities.
///
/// This is the main physics system. It runs every tick for every alive,
/// non-deposit particle. See module-level docs for the complete list of
/// force types.
///
/// ## Implementation Notes
///
/// - Uses a two-pass approach: compute all forces into a temp buffer, then
///   apply to velocities. This avoids aliasing issues.
/// - Neighbor queries use the spatial grid (must be rebuilt first via
///   `rebuild_grid_system`).
/// - World wrapping is handled via `delta_wrapped` for correct distance/direction.
pub fn apply_forces_inner(
    store: &mut ParticleStore,
    grid: &SpatialGrid,
    config: &SimConfig,
    matrices: &SimMatrices,
    vents: &VentList,
    fields: &SimFields,
    day_night: &DayNightState,
    rng: &mut SimRng,
) {
    let n = store.len();
    if n == 0 {
        return;
    }

    let ws = config.world_size;
    let bd = config.bond_distance;
    let ir = config.interaction_radius;
    let ir_sq = ir * ir;
    let repulsion_dist = bd * 0.5; // Below this distance, hard repulsion kicks in
    let repulsion_dist_sq = repulsion_dist * repulsion_dist;

    // -----------------------------------------------------------------------
    // Pass 1: Compute forces into a temporary buffer
    // -----------------------------------------------------------------------
    // We pre-allocate the force buffer. Dead particles get (0,0,0).
    let mut forces: Vec<(f32, f32, f32)> = vec![(0.0, 0.0, 0.0); n];

    // Reusable neighbor buffer to avoid per-particle allocation
    let mut neighbors: Vec<usize> = Vec::with_capacity(128);

    for i in 0..n {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        let mut fx: f32 = 0.0;
        let mut fy: f32 = 0.0;
        let mut fz: f32 = 0.0;

        let ti = store.ptype[i].as_index();
        let my_org = store.organism_id[i];
        let my_sig = store.signature[i];
        let my_role = store.cell_role[i];
        let my_energy = store.energy[i];
        let px = store.x[i];
        let py = store.y[i];
        let pz = store.z[i];

        // -------------------------------------------------------------------
        // A. Neighbor-dependent forces
        // -------------------------------------------------------------------
        grid.query_into(px, py, pz, &mut neighbors);

        for &j in &neighbors {
            if j == i || !store.alive[j] {
                continue;
            }

            // Compute wrapped displacement from i to j
            let (dx, dy, dz) = store.delta_wrapped(i, j, ws);
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq > ir_sq || dist_sq < 1e-10 {
                continue; // Too far or same position
            }

            let dist = dist_sq.sqrt();
            let inv_dist = 1.0 / dist;
            // Normalized direction from i to j
            let nx = dx * inv_dist;
            let ny = dy * inv_dist;
            let nz = dz * inv_dist;

            let tj = store.ptype[j].as_index();
            let j_org = store.organism_id[j];
            let j_sig = store.signature[j];
            let is_bonded = store.bonds[i].contains(&store.id[j]);

            // --- Force 1: Bond spring ---
            if is_bonded {
                // Hooke's law: force proportional to displacement from rest length
                let displacement = dist - bd;
                let bond_str = matrices.bond_str[ti][tj];
                let spring_f = BOND_SPRING_K * bond_str * displacement;
                fx += nx * spring_f;
                fy += ny * spring_f;
                fz += nz * spring_f;
            }

            // --- Force 2: Affinity (non-bonded or in addition to bonds) ---
            if !is_bonded {
                let affinity = matrices.affinity[ti][tj];
                // Affinity falls off with distance (inverse-linear)
                let aff_f = AFFINITY_STRENGTH * affinity * (1.0 - dist / ir);
                fx += nx * aff_f;
                fy += ny * aff_f;
                fz += nz * aff_f;
            }

            // --- Force 3: Hard repulsion (prevent overlap) ---
            if dist_sq < repulsion_dist_sq {
                // Strong repulsive force that increases as particles get closer
                let overlap = 1.0 - dist / repulsion_dist;
                let rep_f = -REPULSION_STRENGTH * overlap * overlap;
                fx += nx * rep_f;
                fy += ny * rep_f;
                fz += nz * rep_f;
            }

            // --- Force 4: Motor predation push ---
            // Motor particles push into foreign organism members
            if store.ptype[i] == ParticleType::Motor
                && my_org >= 0
                && j_org >= 0
                && my_org != j_org
                && !store.is_deposit[j]
            {
                fx += nx * MOTOR_PREDATION_PUSH;
                fy += ny * MOTOR_PREDATION_PUSH;
                fz += nz * MOTOR_PREDATION_PUSH;
            }

            // --- Force 5: Defense role repulsion ---
            // Defense-role particles repel foreign organisms
            if my_role == CellRole::Defense
                && j_org >= 0
                && my_org >= 0
                && my_org != j_org
            {
                let def_f = -DEFENSE_REPULSION * (1.0 - dist / ir);
                fx += nx * def_f;
                fy += ny * def_f;
                fz += nz * def_f;
            }

            // --- Force 6: Immune repulsion ---
            // Different immune signatures cause repulsion between organisms
            if my_org >= 0
                && j_org >= 0
                && my_org != j_org
                && my_sig != 0
                && j_sig != 0
                && my_sig != j_sig
            {
                let imm_f = -IMMUNE_REPULSION * (1.0 - dist / ir);
                fx += nx * imm_f;
                fy += ny * imm_f;
                fz += nz * imm_f;
            }
        }

        // -------------------------------------------------------------------
        // B. Global forces (not neighbor-dependent)
        // -------------------------------------------------------------------

        // --- Force 7: Solar radiation pressure ---
        // Beta particles are pushed in the solar direction
        if store.ptype[i] == ParticleType::Beta {
            let solar_f = day_night.solar_now * SOLAR_FORCE_SCALE;
            fx += config.solar_dir.x * solar_f;
            fy += config.solar_dir.y * solar_f;
            fz += config.solar_dir.z * solar_f;
        }

        // --- Force 8: Digester role extra solar capture ---
        if my_role == CellRole::Digester {
            let dig_f = day_night.solar_now * DIGESTER_SOLAR_BONUS;
            fx += config.solar_dir.x * dig_f;
            fy += config.solar_dir.y * dig_f;
            fz += config.solar_dir.z * dig_f;
        }

        // --- Force 9: Vent attraction (Catalyst particles) ---
        if store.ptype[i] == ParticleType::Catalyst {
            for vent in &vents.0 {
                let vdx = vent.position.x - px;
                let vdy = vent.position.y - py;
                let vdz = vent.position.z - pz;
                let vdist_sq = vdx * vdx + vdy * vdy + vdz * vdz;
                if vdist_sq < VENT_EFFECT_RADIUS * VENT_EFFECT_RADIUS && vdist_sq > 1e-6 {
                    let vdist = vdist_sq.sqrt();
                    let falloff = 1.0 - vdist / VENT_EFFECT_RADIUS;
                    let vf = VENT_ATTRACTION * vent.strength * falloff;
                    fx += (vdx / vdist) * vf;
                    fy += (vdy / vdist) * vf;
                    fz += (vdz / vdist) * vf;
                }
            }
        }

        // --- Force 10: Nutrient gradient (Catalyst in organisms) ---
        if store.ptype[i] == ParticleType::Catalyst && my_org >= 0 {
            let grad = fields.nutrient.gradient(px, py, pz, ws);
            fx += grad.x * NUTRIENT_GRADIENT_STR;
            fy += grad.y * NUTRIENT_GRADIENT_STR;
            fz += grad.z * NUTRIENT_GRADIENT_STR;
        }

        // --- Force 11: Pheromone attraction ---
        // Catalysts follow attractive pheromone gradients
        if store.ptype[i] == ParticleType::Catalyst {
            let grad = fields.phero_attr.gradient(px, py, pz, ws);
            fx += grad.x * PHERO_ATTR_STRENGTH;
            fy += grad.y * PHERO_ATTR_STRENGTH;
            fz += grad.z * PHERO_ATTR_STRENGTH;
        }

        // --- Force 12: Alarm pheromone avoidance ---
        // All organism members flee alarm pheromone
        if my_org >= 0 {
            let grad = fields.phero_alarm.gradient(px, py, pz, ws);
            // Flee = negative gradient direction
            fx -= grad.x * PHERO_ALARM_STRENGTH;
            fy -= grad.y * PHERO_ALARM_STRENGTH;
            fz -= grad.z * PHERO_ALARM_STRENGTH;
        }

        // --- Force 13: Wave field response (Catalyst, Motor) ---
        if store.ptype[i] == ParticleType::Catalyst
            || store.ptype[i] == ParticleType::Motor
        {
            let grad = fields.wave_amp.gradient(px, py, pz, ws);
            fx += grad.x * WAVE_RESPONSE_STR;
            fy += grad.y * WAVE_RESPONSE_STR;
            fz += grad.z * WAVE_RESPONSE_STR;
        }

        // --- Force 14: Metabolite gradient ---
        // All particles follow combined metabolite gradients
        {
            let ga = fields.meta_a.gradient(px, py, pz, ws);
            let gb = fields.meta_b.gradient(px, py, pz, ws);
            let gc = fields.meta_c.gradient(px, py, pz, ws);

            // Type-specific metabolite preference
            let (wa, wb, wc) = match store.ptype[i] {
                ParticleType::Alpha    => (1.0_f32, 0.3, 0.3),
                ParticleType::Beta     => (0.3, 1.0, 0.3),
                ParticleType::Catalyst => (0.5, 0.5, 0.8),
                ParticleType::Data     => (0.3, 0.3, 1.0),
                ParticleType::Membrane => (0.8, 0.2, 0.2),
                ParticleType::Motor    => (0.2, 0.8, 0.5),
            };

            fx += (ga.x * wa + gb.x * wb + gc.x * wc) * METABOLITE_STR;
            fy += (ga.y * wa + gb.y * wb + gc.y * wc) * METABOLITE_STR;
            fz += (ga.z * wa + gb.z * wb + gc.z * wc) * METABOLITE_STR;
        }

        // --- Force 15: Symbol field response ---
        // Organisms with Catalyst members follow their own symbol channel
        if my_org >= 0 && store.symbol_code[i] > 0 {
            let ch = (store.symbol_code[i] - 1) as usize;
            if ch < 8 {
                let grad = fields.symbol[ch].gradient(px, py, pz, ws);
                fx += grad.x * SYMBOL_FIELD_STR;
                fy += grad.y * SYMBOL_FIELD_STR;
                fz += grad.z * SYMBOL_FIELD_STR;
            }
        }

        // --- Force 16: Motor output (signal→force, phase modulated) ---
        // Motor particles convert their signal into directional force,
        // modulated by their phase oscillation
        if store.ptype[i] == ParticleType::Motor {
            let sig = store.signal[i];
            let phase = store.phase[i];

            // Phase-modulated direction (creates circular/spiral motion)
            let cos_p = phase.cos();
            let sin_p = phase.sin();

            // Motor force magnitude
            let mut motor_f = sig * MOTOR_SIGNAL_FORCE;

            // Motor cell role gets a boost
            if my_role == CellRole::MotorCell {
                motor_f *= MOTOR_CELL_BOOST;
            }

            // Apply phase-modulated force in the XZ plane with some Y component
            fx += cos_p * motor_f;
            fy += (sig * 0.5 - 0.25) * motor_f * 0.5; // Slight vertical bias
            fz += sin_p * motor_f;
        }

        // --- Force 17: Epigenetic flight response ---
        // High epi_weight particles flee from nearby alarm pheromone sources
        if store.epi_weight[i] > 1.2 && my_org >= 0 {
            let alarm_val = fields.phero_alarm.sample(px, py, pz, ws);
            if alarm_val > 0.1 {
                let grad = fields.phero_alarm.gradient(px, py, pz, ws);
                let epi_factor = (store.epi_weight[i] - 1.0) * EPI_FLIGHT_STR;
                // Flee direction = negative gradient, scaled by alarm intensity
                fx -= grad.x * epi_factor * alarm_val;
                fy -= grad.y * epi_factor * alarm_val;
                fz -= grad.z * epi_factor * alarm_val;
            }
        }

        // --- Force 18: Brownian motion (thermal jitter) ---
        {
            let brownian = config.temperature * BROWNIAN_SCALE;
            fx += (rng.next() - 0.5) * brownian;
            fy += (rng.next() - 0.5) * brownian;
            fz += (rng.next() - 0.5) * brownian;
        }

        // --- Energy-dependent force scaling ---
        // Low-energy particles have weaker forces (they're "tired")
        if my_energy < 2.0 {
            let energy_scale = (my_energy / 2.0).max(0.1);
            fx *= energy_scale;
            fy *= energy_scale;
            fz *= energy_scale;
        }

        forces[i] = (fx, fy, fz);
    }

    // -----------------------------------------------------------------------
    // Pass 2: Apply accumulated forces to velocities
    // -----------------------------------------------------------------------
    for i in 0..n {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }
        let (fx, fy, fz) = forces[i];
        store.vx[i] += fx;
        store.vy[i] += fy;
        store.vz[i] += fz;
    }
}

pub fn apply_forces_system(
    mut store: ResMut<ParticleStore>,
    grid: Res<SpatialGrid>,
    config: Res<SimConfig>,
    matrices: Res<SimMatrices>,
    vents: Res<VentList>,
    fields: Res<SimFields>,
    day_night: Res<DayNightState>,
    mut rng: ResMut<SimRng>,
) {
    apply_forces_inner(&mut *store, &*grid, &*config, &*matrices, &*vents, &*fields, &*day_night, &mut *rng);
}
