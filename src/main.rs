//! Genesis Engine — Native Bevy application entry point
//!
//! Wires together the simulation, rendering, and UI crates into a single Bevy app.

use bevy::prelude::*;
use genesis_render::GenesisRenderPlugin;
use genesis_sim::GenesisSimPlugin;
use genesis_ui::GenesisUiPlugin;

fn main() {
    // On WASM, install a panic hook that sends Rust panic messages to
    // the browser console instead of swallowing them silently.
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    App::new()
        // ── Core Bevy plugins ──────────────────────────────────────────
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Genesis Engine v6.1".into(),
                resolution: (1280.0, 720.0).into(),
                present_mode: bevy::window::PresentMode::AutoVsync,
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                canvas: Some("#bevy-canvas".to_string()),
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
