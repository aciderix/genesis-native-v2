// ── fields.rs ── Scalar field diffusion, decay, and wave propagation ────────
//
// Runs every tick to evolve the spatial fields that particles interact with.
// Fields provide the medium for chemical signaling, nutrient distribution,
// wave propagation, and metabolite gradients.

use bevy::prelude::*;
use crate::resources::*;
use crate::config::SimConfig;
use crate::particle_store::SimRng;

/// Diffuse and decay all scalar fields each tick.
///
/// Field parameters:
///   • Nutrient (20³):     diffuse 0.03, decay 0.001, regenerate every 20 ticks
///   • Pheromone attract:  diffuse 0.05, decay 0.01
///   • Pheromone alarm:    diffuse 0.08, decay 0.02
///   • Pheromone trail:    diffuse 0.04, decay 0.008
///   • Wave (amp/prev):    2-frame wave equation with 0.98 damping
///   • Metabolites a/b/c:  diffuse 0.04, decay 0.005
///   • Symbol channels:    diffuse 0.06, decay 0.015
pub fn update_fields_inner(
    fields: &mut SimFields,
    counters: &SimCounters,
    config: &SimConfig,
    vents: &VentList,
    rng: &mut SimRng,
) {
    let tick = counters.tick;

    // ── Nutrient field ──────────────────────────────────────────────────
    fields.nutrient.diffuse(0.03);
    fields.nutrient.decay(0.001);

    // ── Pheromone fields ────────────────────────────────────────────────
    fields.phero_attr.diffuse(0.05);
    fields.phero_attr.decay(0.01);

    fields.phero_alarm.diffuse(0.08);
    fields.phero_alarm.decay(0.02);

    fields.phero_trail.diffuse(0.04);
    fields.phero_trail.decay(0.008);

    // ── Wave propagation ────────────────────────────────────────────────
    // Two-frame wave equation: new = 2*current - prev + laplacian*0.25
    // Then damp by 0.98.
    //
    // We swap wave_amp (current) into wave_prev's slot first, then compute
    // the new current frame from the old current (now in wave_prev) and
    // old previous (now in wave_amp).
    {
        let s = fields.wave_amp.size as i32;
        // Swap: after this, wave_prev holds old current, wave_amp holds old previous
        std::mem::swap(&mut fields.wave_amp.data, &mut fields.wave_prev.data);

        for z in 0..s {
            for y in 0..s {
                for x in 0..s {
                    let cur = fields.wave_prev.get(x, y, z); // old current
                    let prev = fields.wave_amp.get(x, y, z); // old previous

                    // 6-neighbor Laplacian
                    let laplacian = fields.wave_prev.get(x - 1, y, z)
                        + fields.wave_prev.get(x + 1, y, z)
                        + fields.wave_prev.get(x, y - 1, z)
                        + fields.wave_prev.get(x, y + 1, z)
                        + fields.wave_prev.get(x, y, z - 1)
                        + fields.wave_prev.get(x, y, z + 1)
                        - 6.0 * cur;

                    let nv = (2.0 * cur - prev + laplacian * 0.25) * 0.98;
                    fields.wave_amp.set(x, y, z, nv);
                }
            }
        }
    }

    // ── Metabolite fields ───────────────────────────────────────────────
    fields.meta_a.diffuse(0.04);
    fields.meta_a.decay(0.005);

    fields.meta_b.diffuse(0.04);
    fields.meta_b.decay(0.005);

    fields.meta_c.diffuse(0.04);
    fields.meta_c.decay(0.005);

    // ── Symbol channels ─────────────────────────────────────────────────
    for ch in fields.symbol.iter_mut() {
        ch.diffuse(0.06);
        ch.decay(0.015);
    }

    // ── Nutrient regeneration ───────────────────────────────────────────
    // Every 20 ticks, inject nutrients at vent positions and a few random spots.
    if tick % 20 == 0 {
        let ws = config.world_size;
        let ns = fields.nutrient.size as i32;

        // Vent nutrient injection
        for vent in vents.0.iter() {
            let vx = vent.position.x;
            let vy = vent.position.y;
            let vz = vent.position.z;
            // Convert world position to grid coordinates
            let gx = ((vx / ws + 0.5) * ns as f32) as i32;
            let gy = ((vy / ws + 0.5) * ns as f32) as i32;
            let gz = ((vz / ws + 0.5) * ns as f32) as i32;
            // Inject in a small radius around the vent
            let r = (vent.radius / ws * ns as f32).ceil() as i32;
            for dz in -r..=r {
                for dy in -r..=r {
                    for dx in -r..=r {
                        let d2 = dx * dx + dy * dy + dz * dz;
                        if d2 <= r * r {
                            let strength = vent.strength * 0.05
                                * (1.0 - (d2 as f32).sqrt() / r.max(1) as f32);
                            fields.nutrient.add(gx + dx, gy + dy, gz + dz, strength);
                        }
                    }
                }
            }
        }

        // Random nutrient spots (3 per regeneration cycle)
        for _ in 0..3 {
            let rx = (rng.next() * ns as f32) as i32;
            let ry = (rng.next() * ns as f32) as i32;
            let rz = (rng.next() * ns as f32) as i32;
            fields.nutrient.add(rx, ry, rz, 0.2);
        }
    }
}

pub fn update_fields_system(
    mut fields: ResMut<SimFields>,
    counters: Res<SimCounters>,
    config: Res<SimConfig>,
    vents: Res<VentList>,
    mut rng: ResMut<SimRng>,
) {
    update_fields_inner(&mut *fields, &*counters, &*config, &*vents, &mut *rng);
}
