//! # Genesis UI
//!
//! Egui-based user interface for Genesis 2.0.
//! Displays key metrics: population, groups, species, assembly index,
//! innovation rate, phylogenetic diversity.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use genesis_core::metrics;
use genesis_sim::config::SimConfig;
use genesis_sim::particle_store::ParticleStore;
use genesis_sim::resources::*;

// ──────────────────────────────────────────────────────────────────────────────
// Plugin
// ──────────────────────────────────────────────────────────────────────────────

/// Top-level UI plugin. Adds egui, the shared [`UiState`] resource, and the
/// main `ui_system` that draws panels each frame.
pub struct GenesisUiPlugin;

impl Plugin for GenesisUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .init_resource::<UiState>()
            .add_systems(Update, ui_system);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// UI State
// ──────────────────────────────────────────────────────────────────────────────

/// Persistent UI state.
#[derive(Resource)]
pub struct UiState {
    pub show_hud: bool,
    pub show_controls: bool,
    pub show_charts: bool,
    /// Set to `true` when the user presses the Reset button.
    pub reset_requested: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_hud: true,
            show_controls: true,
            show_charts: true,
            reset_requested: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Theme
// ──────────────────────────────────────────────────────────────────────────────

/// Apply a dark, Genesis-branded colour scheme.
fn apply_dark_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let visuals = &mut style.visuals;

    visuals.dark_mode = true;
    visuals.panel_fill = egui::Color32::from_rgba_premultiplied(18, 18, 24, 240);
    visuals.window_fill = egui::Color32::from_rgba_premultiplied(22, 22, 30, 245);
    visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 230));
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 30, 42);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 40, 56);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 75);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 70, 95);
    visuals.selection.bg_fill = egui::Color32::from_rgb(60, 90, 160);

    ctx.set_style(style);
}

// ──────────────────────────────────────────────────────────────────────────────
// Keyboard shortcuts
// ──────────────────────────────────────────────────────────────────────────────

fn handle_keyboard(
    keyboard: &ButtonInput<KeyCode>,
    paused: &mut SimPaused,
    ui_state: &mut UiState,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        paused.0 = !paused.0;
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        ui_state.show_hud = !ui_state.show_hud;
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        ui_state.show_charts = !ui_state.show_charts;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        ui_state.reset_requested = true;
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Compute metrics from the store
// ──────────────────────────────────────────────────────────────────────────────

struct LiveMetrics {
    population: usize,
    num_groups: usize,
    num_species_approx: usize,
    avg_assembly_index: f32,
    max_assembly_index: usize,
    innovation_rate: f32,
    phylo_diversity: f32,
    total_energy: f32,
    avg_genome_length: f32,
}

fn compute_live_metrics(store: &ParticleStore, metrics_history: &MetricsHistory) -> LiveMetrics {
    let population = store.population();

    // Count groups (unique positive group_ids among alive particles)
    let mut group_set = std::collections::HashSet::new();
    for i in 0..store.count {
        if store.alive[i] && store.group_ids[i] >= 0 {
            group_set.insert(store.group_ids[i]);
        }
    }
    let num_groups = group_set.len();

    // Compute assembly indices and collect genome data
    let mut ai_values = Vec::new();
    let mut total_energy = 0.0f32;
    let mut total_genome_len = 0usize;
    let mut alive_genomes = Vec::new();
    let mut alive_parent_ids = Vec::new();
    let mut alive_generations = Vec::new();

    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        let ai = store.genomes[i].assembly_index();
        ai_values.push(ai);
        total_energy += store.energy[i];
        total_genome_len += store.genomes[i].reactions.len();
        alive_genomes.push(store.genomes[i].clone());
        alive_parent_ids.push(store.parent_ids[i]);
        alive_generations.push(store.generations[i]);
    }

    let avg_assembly_index = if ai_values.is_empty() {
        0.0
    } else {
        ai_values.iter().sum::<usize>() as f32 / ai_values.len() as f32
    };

    let max_assembly_index = ai_values.iter().copied().max().unwrap_or(0);

    let avg_genome_length = if population == 0 {
        0.0
    } else {
        total_genome_len as f32 / population as f32
    };

    // Approximate species: cluster by assembly index value
    let mut species_set = std::collections::HashSet::new();
    for &ai in &ai_values {
        species_set.insert(ai);
    }
    let num_species_approx = species_set.len();

    // Innovation rate from metrics history
    let (innovation_count, _) = metrics::innovation_count(&alive_genomes, &metrics_history.known_reactions);
    let innovation_rate = if population == 0 {
        0.0
    } else {
        innovation_count as f32 / population as f32
    };

    // Phylogenetic diversity
    let phylo_diversity = metrics::phylogenetic_diversity(&alive_parent_ids, &alive_generations);

    LiveMetrics {
        population,
        num_groups,
        num_species_approx,
        avg_assembly_index,
        max_assembly_index,
        innovation_rate,
        phylo_diversity,
        total_energy,
        avg_genome_length,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Main UI system
// ──────────────────────────────────────────────────────────────────────────────

/// The single Bevy system that drives the entire UI each frame.
fn ui_system(
    mut contexts: EguiContexts,
    store: Res<ParticleStore>,
    config: Res<SimConfig>,
    tick: Res<SimTick>,
    mut paused: ResMut<SimPaused>,
    metrics_history: Res<MetricsHistory>,
    mut ui_state: ResMut<UiState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    // Keyboard shortcuts
    handle_keyboard(&keyboard, &mut paused, &mut ui_state);

    let ctx = contexts.ctx_mut();
    apply_dark_theme(ctx);

    let fps = 1.0 / time.delta_secs().max(1e-6);
    let live = compute_live_metrics(&store, &metrics_history);

    // ── 1. Top HUD bar ─────────────────────────────────────────────────
    if ui_state.show_hud {
        egui::TopBottomPanel::top("hud_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;

                // Tick
                ui.colored_label(egui::Color32::from_rgb(120, 180, 255), "⏱");
                ui.label(format!("Tick {}", tick.0));

                ui.separator();

                // Population
                ui.colored_label(egui::Color32::from_rgb(180, 140, 255), "●");
                ui.label(format!("{}", live.population));

                ui.separator();

                // Groups
                ui.colored_label(egui::Color32::from_rgb(100, 255, 140), "⬡");
                ui.label(format!("{} groups", live.num_groups));

                ui.separator();

                // Species (approx)
                ui.colored_label(egui::Color32::from_rgb(255, 200, 80), "🧬");
                ui.label(format!("~{} species", live.num_species_approx));

                ui.separator();

                // Assembly Index
                ui.colored_label(egui::Color32::from_rgb(200, 140, 255), "AI");
                ui.label(format!("{:.1}/{}", live.avg_assembly_index, live.max_assembly_index));

                ui.separator();

                // FPS
                ui.label(format!("FPS: {:.0}", fps));

                // Right: Pause/Play
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pause_text = if paused.0 { "▶ Play" } else { "⏸ Pause" };
                    if ui.button(pause_text).clicked() {
                        paused.0 = !paused.0;
                    }
                });
            });
        });
    }

    // ── 2. Left panel: Controls + Metrics ──────────────────────────────
    if ui_state.show_controls {
        egui::SidePanel::left("controls_panel")
            .default_width(240.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("⚙ Controls");
                ui.separator();
                ui.add_space(4.0);

                // Pause / Play toggle
                let pause_label = if paused.0 {
                    "▶  Resume"
                } else {
                    "⏸  Pause"
                };
                if ui.button(pause_label).clicked() {
                    paused.0 = !paused.0;
                }
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // World info
                ui.colored_label(egui::Color32::from_rgb(140, 200, 255), "🌍 World");
                ui.add_space(2.0);
                ui.label(format!("Size: {:.0} × {:.0}", config.world_width, config.world_height));
                ui.label(format!("Mutation rate: {:.3}", config.mutation_rate));
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // ── Metrics display ──────────────────────────────────────
                ui.colored_label(egui::Color32::from_rgb(100, 255, 200), "📊 Metrics");
                ui.add_space(2.0);

                ui.label(format!("Population:     {}", live.population));
                ui.label(format!("Groups:         {}", live.num_groups));
                ui.label(format!("Species (≈):    {}", live.num_species_approx));
                ui.add_space(4.0);

                // Assembly Index
                ui.colored_label(egui::Color32::from_rgb(200, 140, 255), "Assembly Index");
                ui.label(format!("  Average: {:.2}", live.avg_assembly_index));
                ui.label(format!("  Maximum: {}", live.max_assembly_index));
                ui.add_space(4.0);

                // Innovation Rate
                ui.colored_label(egui::Color32::from_rgb(255, 220, 100), "Innovation Rate");
                ui.label(format!("  {:.4}", live.innovation_rate));
                ui.add_space(4.0);

                // Phylogenetic Diversity
                ui.colored_label(egui::Color32::from_rgb(100, 200, 255), "Phylo Diversity");
                ui.label(format!("  {:.2}", live.phylo_diversity));
                ui.add_space(4.0);

                // Genome info
                ui.colored_label(egui::Color32::from_rgb(180, 180, 255), "Genome Stats");
                ui.label(format!("  Avg length: {:.1}", live.avg_genome_length));
                ui.label(format!("  Total energy: {:.0}", live.total_energy));
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Reset button
                if ui
                    .add(
                        egui::Button::new("🔄 Reset Simulation")
                            .fill(egui::Color32::from_rgb(140, 40, 40)),
                    )
                    .clicked()
                {
                    ui_state.reset_requested = true;
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "Panels");
                ui.checkbox(&mut ui_state.show_hud, "HUD  (H)");
                ui.checkbox(&mut ui_state.show_charts, "Charts  (C)");
            });
    }

    // ── 3. Bottom panel: Mini-charts ────────────────────────────────────
    if ui_state.show_charts {
        egui::TopBottomPanel::bottom("charts_panel")
            .default_height(110.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Population chart
                    let pop_data: Vec<f32> = metrics_history
                        .snapshots
                        .iter()
                        .map(|s| s.population as f32)
                        .collect();
                    draw_mini_chart(
                        ui,
                        "Population",
                        &pop_data,
                        egui::Color32::from_rgb(100, 255, 140),
                    );
                    ui.add_space(8.0);

                    // Assembly Index chart
                    let ai_data: Vec<f32> = metrics_history
                        .snapshots
                        .iter()
                        .map(|s| s.avg_ai)
                        .collect();
                    draw_mini_chart(
                        ui,
                        "Avg AI",
                        &ai_data,
                        egui::Color32::from_rgb(200, 140, 255),
                    );
                    ui.add_space(8.0);

                    // Innovation Rate chart
                    let innov_data: Vec<f32> = metrics_history
                        .snapshots
                        .iter()
                        .map(|s| s.innovation_rate)
                        .collect();
                    draw_mini_chart(
                        ui,
                        "Innovation",
                        &innov_data,
                        egui::Color32::from_rgb(255, 220, 80),
                    );
                    ui.add_space(8.0);

                    // Phylogenetic Diversity chart
                    let pd_data: Vec<f32> = metrics_history
                        .snapshots
                        .iter()
                        .map(|s| s.phylo_diversity)
                        .collect();
                    draw_mini_chart(
                        ui,
                        "Phylo Div",
                        &pd_data,
                        egui::Color32::from_rgb(100, 200, 255),
                    );
                    ui.add_space(8.0);

                    // Energy chart
                    let energy_data: Vec<f32> = metrics_history
                        .snapshots
                        .iter()
                        .map(|s| s.total_energy as f32)
                        .collect();
                    draw_mini_chart(
                        ui,
                        "Energy",
                        &energy_data,
                        egui::Color32::from_rgb(255, 180, 80),
                    );
                });
            });
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Mini chart drawing
// ──────────────────────────────────────────────────────────────────────────────

/// Draw a single mini line-chart (200 × 80 px).
fn draw_mini_chart(ui: &mut egui::Ui, title: &str, data: &[f32], color: egui::Color32) {
    let chart_w = 200.0_f32;
    let chart_h = 80.0_f32;

    ui.vertical(|ui| {
        // Title
        ui.colored_label(color, title);

        // Allocate drawing area
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(chart_w, chart_h), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(24, 24, 34));

        if data.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "no data",
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(100, 100, 120),
            );
            return;
        }

        // Determine visible window (last N points that fit)
        let max_points = chart_w as usize;
        let start = if data.len() > max_points {
            data.len() - max_points
        } else {
            0
        };
        let visible = &data[start..];

        // Y range
        let y_min = visible
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min)
            .min(0.0);
        let y_max = visible
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max)
            .max(y_min + 1.0);
        let y_range = y_max - y_min;

        // Build points
        let n = visible.len() as f32;
        let points: Vec<egui::Pos2> = visible
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let x = rect.min.x + (i as f32 / n) * chart_w;
                let y = rect.max.y - ((v - y_min) / y_range) * chart_h;
                egui::pos2(x, y)
            })
            .collect();

        // Draw line
        if points.len() >= 2 {
            let stroke = egui::Stroke::new(1.5, color);
            for window in points.windows(2) {
                painter.line_segment([window[0], window[1]], stroke);
            }
        }

        // Current value label
        if let Some(&last) = visible.last() {
            painter.text(
                egui::pos2(rect.max.x - 4.0, rect.min.y + 4.0),
                egui::Align2::RIGHT_TOP,
                format!("{:.0}", last),
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgb(200, 200, 210),
            );
        }
    });
}
