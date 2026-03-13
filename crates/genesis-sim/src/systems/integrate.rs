//! Velocity integration system.
//!
//! Applies velocity to positions (Euler integration), then:
//! - Applies drag to velocities
//! - Caps maximum speed
//! - Wraps positions toroidally at world boundaries
//! - Increments particle age
//!
//! This runs AFTER the forces system has updated velocities.

use bevy::prelude::*;
use crate::particle_store::ParticleStore;
use crate::config::SimConfig;

/// Maximum allowed speed for any particle (world-units per tick).
///
/// Prevents simulation instability from runaway velocities. Matches the
/// TypeScript constant `MAX_SPEED = 1.5`.
const MAX_SPEED: f32 = 1.5;

/// Drag coefficient applied each tick: `velocity *= (1.0 - DRAG)`.
///
/// Provides gradual deceleration to prevent perpetual motion.
/// Matches TS `DRAG = 0.03`.
const DRAG: f32 = 0.03;

/// Euler-integrate velocities into positions, then apply drag, speed cap,
/// toroidal wrapping, and age increment.
///
/// ## Algorithm (per alive particle)
///
/// 1. **Position update**: `pos += vel`
/// 2. **Drag**: `vel *= (1.0 - DRAG)`
/// 3. **Speed cap**: if `|vel| > MAX_SPEED`, scale vel down to `MAX_SPEED`
/// 4. **Toroidal wrap**: if `pos.x > world_size`, wrap to `-world_size`, etc.
/// 5. **Age increment**: `age += 1`
///
/// Deposits are integrated normally (they just sit there with ~zero velocity),
/// but their age still increments.
pub fn integrate_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
) {
    let ws = config.world_size;
    let ws2 = ws * 2.0; // full world diameter for wrapping
    let n = store.len();

    for i in 0..n {
        if !store.alive[i] {
            continue;
        }

        // --- 1. Position update (Euler integration) ---
        store.x[i] += store.vx[i];
        store.y[i] += store.vy[i];
        store.z[i] += store.vz[i];

        // --- 2. Apply drag ---
        let drag_factor = 1.0 - DRAG;
        store.vx[i] *= drag_factor;
        store.vy[i] *= drag_factor;
        store.vz[i] *= drag_factor;

        // --- 3. Speed cap ---
        let speed_sq = store.vx[i] * store.vx[i]
            + store.vy[i] * store.vy[i]
            + store.vz[i] * store.vz[i];

        if speed_sq > MAX_SPEED * MAX_SPEED {
            let speed = speed_sq.sqrt();
            let scale = MAX_SPEED / speed;
            store.vx[i] *= scale;
            store.vy[i] *= scale;
            store.vz[i] *= scale;
        }

        // --- 4. Toroidal wrapping ---
        // World extends from -world_size to +world_size on each axis.
        if store.x[i] > ws {
            store.x[i] -= ws2;
        } else if store.x[i] < -ws {
            store.x[i] += ws2;
        }

        if store.y[i] > ws {
            store.y[i] -= ws2;
        } else if store.y[i] < -ws {
            store.y[i] += ws2;
        }

        if store.z[i] > ws {
            store.z[i] -= ws2;
        } else if store.z[i] < -ws {
            store.z[i] += ws2;
        }

        // --- 5. Age increment ---
        store.age[i] = store.age[i].wrapping_add(1);
    }
}

pub fn integrate_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
) {
    integrate_inner(&mut *store, &*config);
}
