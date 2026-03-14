//! # Genesis Render
//!
//! Bevy 0.15 2D rendering plugin for Genesis 2.0.
//!
//! Uses Gizmos for lightweight rendering of particles and bonds.
//! Particle colours are derived from their chemical concentrations,
//! and radius from total chemical load.

use bevy::prelude::*;
use genesis_core::chemistry;
use genesis_sim::config::SimConfig;
use genesis_sim::particle_store::ParticleStore;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Top-level render plugin — add this to the Bevy [`App`] to get 2-D
/// visualisation of the running simulation via Gizmos.
pub struct GenesisRenderPlugin;

impl Plugin for GenesisRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, render_system);
    }
}

// ---------------------------------------------------------------------------
// Render system
// ---------------------------------------------------------------------------

/// Every frame, draw all alive particles as coloured circles and bonds as
/// semi-transparent lines using Bevy's Gizmos API.
fn render_system(
    store: Res<ParticleStore>,
    config: Res<SimConfig>,
    mut gizmos: Gizmos,
) {
    let half_w = config.world_width / 2.0;
    let half_h = config.world_height / 2.0;

    // Draw particles
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        let color_rgb = chemistry::particle_color(&store.chem[i]);
        let radius = chemistry::particle_radius(&store.chem[i]);
        let pos = Vec2::new(store.x[i] - half_w, store.y[i] - half_h);

        // Ensure minimum brightness so particles are always visible
        let r = color_rgb[0].max(0.15);
        let g = color_rgb[1].max(0.15);
        let b = color_rgb[2].max(0.15);
        let color = Color::srgb(r, g, b);

        gizmos.circle_2d(pos, radius * 5.0, color);
    }

    // Draw bonds
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        for &(j, strength) in &store.bonds[i] {
            // Only draw each bond once (i < j) and skip invalid references
            if j <= i || j >= store.count || !store.alive[j] {
                continue;
            }
            let p1 = Vec2::new(store.x[i] - half_w, store.y[i] - half_h);
            let p2 = Vec2::new(store.x[j] - half_w, store.y[j] - half_h);
            let alpha = strength.clamp(0.1, 1.0);
            gizmos.line_2d(p1, p2, Color::srgba(0.5, 0.5, 0.5, alpha));
        }
    }
}
