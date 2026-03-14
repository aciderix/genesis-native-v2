use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::EnvironmentFields;
use genesis_core::cell_role::CellRole;
use genesis_core::chemistry::NUM_CHEMICALS;
use genesis_core::genome::{BehaviorAction, SensorCondition};

/// Evaluate behavior rules for each particle and apply actions.
///
/// Runs after sensing_system. Actions modify velocity and signal emissions.
pub fn behavior_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    env: Res<EnvironmentFields>,
) {
    let count = store.count;

    // Clear behavior deltas
    for i in 0..count {
        store.behavior_vx[i] = 0.0;
        store.behavior_vy[i] = 0.0;
        store.signal_emit[i].clear();
    }

    for i in 0..count {
        if !store.alive[i] {
            continue;
        }

        // Clone behaviors to avoid borrow issues
        let behaviors = store.genomes[i].behaviors.clone();
        let sensor = store.sensors[i].clone();
        let role = store.roles[i];

        for rule in &behaviors {
            // Evaluate condition
            let condition_met = evaluate_condition(&rule.condition, &sensor, role);
            if !condition_met {
                continue;
            }

            // Execute action
            let w = rule.weight;
            match &rule.action {
                BehaviorAction::MoveToward(ch) => {
                    if *ch < NUM_CHEMICALS {
                        // Compute environment gradient for this channel
                        let (gx, gy) = env.world_to_grid(store.x[i], store.y[i]);
                        let grad = env_gradient(&env, *ch, gx, gy);
                        store.behavior_vx[i] += grad.0 * w * 10.0;
                        store.behavior_vy[i] += grad.1 * w * 10.0;
                    }
                }
                BehaviorAction::MoveAway(ch) => {
                    if *ch < NUM_CHEMICALS {
                        let (gx, gy) = env.world_to_grid(store.x[i], store.y[i]);
                        let grad = env_gradient(&env, *ch, gx, gy);
                        store.behavior_vx[i] -= grad.0 * w * 10.0;
                        store.behavior_vy[i] -= grad.1 * w * 10.0;
                    }
                }
                BehaviorAction::Emit(ch, amount) => {
                    if *ch < NUM_CHEMICALS {
                        store.signal_emit[i].push((*ch, amount * w));
                    }
                }
                BehaviorAction::SeekNeighbor => {
                    if sensor.nearest_distance < f32::MAX {
                        store.behavior_vx[i] += sensor.nearest_dir[0] * w * 5.0;
                        store.behavior_vy[i] += sensor.nearest_dir[1] * w * 5.0;
                    }
                }
                BehaviorAction::FleeNeighbor => {
                    if sensor.nearest_distance < f32::MAX {
                        store.behavior_vx[i] -= sensor.nearest_dir[0] * w * 5.0;
                        store.behavior_vy[i] -= sensor.nearest_dir[1] * w * 5.0;
                    }
                }
                BehaviorAction::BoostMotor(amount) => {
                    // Boost existing velocity direction
                    let speed = (store.vx[i] * store.vx[i] + store.vy[i] * store.vy[i]).sqrt();
                    if speed > 0.001 {
                        store.behavior_vx[i] += (store.vx[i] / speed) * amount * w;
                        store.behavior_vy[i] += (store.vy[i] / speed) * amount * w;
                    }
                }
                BehaviorAction::FollowSignal => {
                    store.behavior_vx[i] += sensor.signal_gradient[0] * w * 15.0;
                    store.behavior_vy[i] += sensor.signal_gradient[1] * w * 15.0;
                }
            }
        }
    }

    // Apply behavior velocities to actual velocities
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        store.vx[i] += store.behavior_vx[i] * config.dt;
        store.vy[i] += store.behavior_vy[i] * config.dt;
    }
}

/// Evaluate a sensor condition against the current sensor input.
fn evaluate_condition(cond: &SensorCondition, sensor: &crate::particle_store::SensorInput, role: CellRole) -> bool {
    match cond {
        SensorCondition::ChemAbove(ch, threshold) => {
            *ch < NUM_CHEMICALS && sensor.local_chem[*ch] > *threshold
        }
        SensorCondition::ChemBelow(ch, threshold) => {
            *ch < NUM_CHEMICALS && sensor.local_chem[*ch] < *threshold
        }
        SensorCondition::EnergyAbove(threshold) => sensor.energy > *threshold,
        SensorCondition::EnergyBelow(threshold) => sensor.energy < *threshold,
        SensorCondition::NeighborCountAbove(n) => sensor.neighbor_count > *n,
        SensorCondition::NeighborCountBelow(n) => sensor.neighbor_count < *n,
        SensorCondition::HasBond => sensor.bond_count > 0,
        SensorCondition::NoBond => sensor.bond_count == 0,
        SensorCondition::GroupSizeAbove(n) => sensor.group_size > *n,
        SensorCondition::SignalAbove(threshold) => {
            sensor.env_chem[3] > *threshold // Signal channel is 3
        }
        SensorCondition::RoleIs(r) => role == *r,
        SensorCondition::Always => true,
    }
}

/// Compute the gradient of an environment field at a grid position.
fn env_gradient(env: &crate::resources::EnvironmentFields, ch: usize, gx: usize, gy: usize) -> (f32, f32) {
    let gx_plus = if gx + 1 < env.width { env.get(ch, gx + 1, gy) } else { 0.0 };
    let gx_minus = if gx > 0 { env.get(ch, gx - 1, gy) } else { 0.0 };
    let gy_plus = if gy + 1 < env.height { env.get(ch, gx, gy + 1) } else { 0.0 };
    let gy_minus = if gy > 0 { env.get(ch, gx, gy - 1) } else { 0.0 };
    ((gx_plus - gx_minus) * 0.5, (gy_plus - gy_minus) * 0.5)
}
