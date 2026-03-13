//! Genesis Engine — Native Bevy application entry point
//!
//! Wires together the simulation, rendering, and UI crates into a single Bevy app.

use bevy::prelude::*;
use genesis_render::GenesisRenderPlugin;
use genesis_sim::GenesisSimPlugin;
use genesis_ui::GenesisUiPlugin;

fn main() {
    App::new()
        // ── Core Bevy plugins ──────────────────────────────────────────
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Genesis Engine v6.1".into(),
                resolution: (1600.0, 900.0).into(),
                present_mode: bevy::window::PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        // ── Genesis crates ─────────────────────────────────────────────
        .add_plugins(GenesisSimPlugin::default())
        .add_plugins(GenesisRenderPlugin)
        .add_plugins(GenesisUiPlugin)
        // ── Launch ─────────────────────────────────────────────────────
        .run();
}
