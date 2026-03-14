use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::{ParticleStore, SensorInput};
use crate::resources::EnvironmentFields;
use crate::util::SpatialGrid;
use genesis_core::chemistry::NUM_CHEMICALS;

/// Populate SensorInput for each alive particle.
/// Also rebuilds the SpatialGrid for use by subsequent systems (forces, bonds).
///
/// Runs before behavior_system so particles have fresh sensor data
/// to evaluate their behavior rules.
pub fn sensing_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    env: Res<EnvironmentFields>,
    mut grid: ResMut<SpatialGrid>,
) {
    let count = store.count;

    // ── Rebuild spatial grid (shared with forces_system, bonds_system) ───
    grid.clear();
    for i in 0..count {
        if store.alive[i] {
            grid.insert(i, store.x[i], store.y[i]);
        }
    }

    // Pre-compute sensor data for all particles
    let mut sensor_data: Vec<SensorInput> = vec![SensorInput::default(); count];
    let mut neighbors_buf = Vec::new();

    for i in 0..count {
        if !store.alive[i] {
            continue;
        }

        // Local chemistry and energy
        sensor_data[i].local_chem = store.chem[i];
        sensor_data[i].energy = store.energy[i];
        sensor_data[i].bond_count = store.bonds[i].len();

        // Environment chemistry at position
        let (gx, gy) = env.world_to_grid(store.x[i], store.y[i]);
        for k in 0..NUM_CHEMICALS {
            sensor_data[i].env_chem[k] = env.get(k, gx, gy);
        }

        // Signal gradient (channel 3) — central differences on environment grid
        let signal_ch = 3;
        let sx_plus = if (gx + 1) < env.width { env.get(signal_ch, gx + 1, gy) } else { 0.0 };
        let sx_minus = if gx > 0 { env.get(signal_ch, gx - 1, gy) } else { 0.0 };
        let sy_plus = if (gy + 1) < env.height { env.get(signal_ch, gx, gy + 1) } else { 0.0 };
        let sy_minus = if gy > 0 { env.get(signal_ch, gx, gy - 1) } else { 0.0 };
        sensor_data[i].signal_gradient[0] = (sx_plus - sx_minus) * 0.5;
        sensor_data[i].signal_gradient[1] = (sy_plus - sy_minus) * 0.5;

        // Neighbor data: use spatial grid for O(n) lookups
        grid.query_into(store.x[i], store.y[i], &mut neighbors_buf);

        let mut neighbor_count = 0usize;
        let mut neighbor_chem_sum = [0.0f32; NUM_CHEMICALS];
        let mut nearest_dist = f32::MAX;
        let mut nearest_dx = 0.0f32;
        let mut nearest_dy = 0.0f32;
        let r2 = config.sensing_radius * config.sensing_radius;

        for &j in &neighbors_buf {
            if i == j || !store.alive[j] {
                continue;
            }
            let dx = store.x[j] - store.x[i];
            let dy = store.y[j] - store.y[i];
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > r2 {
                continue;
            }
            let dist = dist_sq.sqrt();
            neighbor_count += 1;
            for k in 0..NUM_CHEMICALS {
                neighbor_chem_sum[k] += store.chem[j][k];
            }
            if dist < nearest_dist {
                nearest_dist = dist;
                nearest_dx = dx;
                nearest_dy = dy;
            }
        }

        sensor_data[i].neighbor_count = neighbor_count;
        sensor_data[i].nearest_distance = nearest_dist;
        if nearest_dist < f32::MAX && nearest_dist > 0.001 {
            sensor_data[i].nearest_dir = [nearest_dx / nearest_dist, nearest_dy / nearest_dist];
        }
        if neighbor_count > 0 {
            let n = neighbor_count as f32;
            for k in 0..NUM_CHEMICALS {
                sensor_data[i].neighbor_avg_chem[k] = neighbor_chem_sum[k] / n;
            }
        }

        // Group size
        let gid = store.group_ids[i];
        if gid >= 0 {
            let mut gsize = 0usize;
            for j in 0..count {
                if store.alive[j] && store.group_ids[j] == gid {
                    gsize += 1;
                }
            }
            sensor_data[i].group_size = gsize;
        }
    }

    // Write back
    store.sensors = sensor_data;
}
