//! # Genesis UI
//!
//! Egui-based user interface for the Genesis Engine simulation.
//! Provides HUD overlay, control panels, particle inspector, mini-charts,
//! and a colour-coded event log — all rendered every frame via `bevy_egui`.

use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use genesis_sim::components::{CellRole, ParticleType, NUM_TYPES};
use genesis_sim::config::SimConfig;
use genesis_sim::particle_store::ParticleStore;
use genesis_sim::resources::*;

// ──────────────────────────────────────────────────────────────────────────────
// Plugin
// ──────────────────────────────────────────────────────────────────────────────

/// Top-level UI plugin.  Adds egui, the shared [`UiState`] resource, and the
/// main `ui_system` that draws every panel each frame.
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

/// Persistent UI state: which panels are visible and which particle (if any)
/// is selected for inspection.
#[derive(Resource)]
pub struct UiState {
    pub show_hud: bool,
    pub show_charts: bool,
    pub show_inspector: bool,
    pub show_events: bool,
    pub show_controls: bool,
    pub selected_particle: Option<usize>,
    /// Set to `true` when the user presses the Reset button.  The simulation
    /// should check this flag, act on it, and clear it.
    pub reset_requested: bool,
    /// Auto-detected: true when running on a small screen (mobile).
    pub is_mobile: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_hud: true,
            show_charts: true,
            show_inspector: false,
            show_events: true,
            show_controls: true,
            selected_particle: None,
            reset_requested: false,
            is_mobile: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Era detection
// ──────────────────────────────────────────────────────────────────────────────

/// Human-readable era label derived from current simulation statistics.
/// Matches the web version's 14-era detection with identical thresholds.
fn detect_era(
    stats: &SimStats,
    counters: &SimCounters,
    store: &ParticleStore,
    org_reg: &OrganismRegistry,
    active_symbols: &ActiveSymbolCodes,
    active_genes: &ActiveGeneCount,
    metabolite_flow: &MetaboliteFlowRate,
    active_memes: &ActiveMemes,
    metacog_count: &MetaCogOrgCount,
    build_count: &BuildStructureCount,
) -> &'static str {
    // P4 eras (highest first) — matches web getEra() exactly
    if metacog_count.0 >= 3 {
        return "Cognitive Age";
    }
    if counters.total_symbiogenesis >= 3 {
        return "Symbiotic Age";
    }
    if active_memes.0.len() >= 4 {
        return "Cultural Age";
    }
    let has_tool = org_reg.organisms.values().any(|o| o.tool_use_count > 0);
    if has_tool && build_count.0 >= 2 {
        return "Tool Age";
    }
    // P3 eras
    if active_symbols.0.len() >= 5 {
        return "Symbolic Age";
    }
    let spec_orgs = org_reg.organisms.values().filter(|o| o.specialization > 0.4).count();
    if spec_orgs >= 3 {
        return "Specialized Age";
    }
    // P2 eras
    if active_genes.0 >= 10 && metabolite_flow.0 > 5.0 {
        return "Genetic Age";
    }
    if metabolite_flow.0 > 8.0 {
        return "Metabolic Age";
    }
    // Original eras
    let dep_count = store.deposit_count();
    let max_col_size = org_reg.organisms.values()
        .filter(|o| o.colony_id >= 0)
        .count();
    if dep_count > 30 && max_col_size >= 3 {
        return "Construction Age";
    }
    if stats.colony_count > 0 || max_col_size >= 2 {
        return "Colonial Era";
    }
    if counters.total_pred > 5 {
        return "Predatory Age";
    }
    if counters.total_repro > 3 {
        return "Replication Era";
    }
    if stats.organism_count > 0 {
        return "Proto-life";
    }
    if store.has_any_bonds() {
        return "Chemical Age";
    }
    "Primordial Soup"
}

// ──────────────────────────────────────────────────────────────────────────────
// Colours & helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Map a `ParticleType` to an egui display colour (matches web TYPE_COLORS).
fn type_color(pt: &ParticleType) -> egui::Color32 {
    match pt {
        ParticleType::Alpha => egui::Color32::from_rgb(192, 192, 192),    // #C0C0C0 Silver
        ParticleType::Beta => egui::Color32::from_rgb(255, 215, 0),       // #FFD700 Gold
        ParticleType::Catalyst => egui::Color32::from_rgb(80, 200, 120),  // #50C878 Emerald
        ParticleType::Data => egui::Color32::from_rgb(65, 105, 225),      // #4169E1 Royal Blue
        ParticleType::Membrane => egui::Color32::from_rgb(155, 89, 182),  // #9B59B6 Purple
        ParticleType::Motor => egui::Color32::from_rgb(231, 76, 60),      // #E74C3C Red
    }
}

/// Colour for the energy bar — green → yellow → red.
fn energy_color(fraction: f32) -> egui::Color32 {
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.5 {
        let t = (f - 0.5) * 2.0;
        egui::Color32::from_rgb((255.0 * (1.0 - t)) as u8, 220, 50)
    } else {
        let t = f * 2.0;
        egui::Color32::from_rgb(230, (220.0 * t) as u8, 50)
    }
}

/// Emoji prefix for event types.
fn event_emoji(kind: &str) -> &'static str {
    match kind {
        "bond" => "🔗",
        "organism" => "🧬",
        "reproduction" => "🔄",
        "death" => "💀",
        "predation" => "⚔️",
        "colony" => "🏘️",
        "milestone" => "⭐",
        "culture" => "🎭",
        "metacog" => "🧠",
        "symbiogenesis" => "🔀",
        "sexual" => "❤️",
        _ => "📋",
    }
}

/// Colour tint for event log entries.
fn event_color(kind: &str) -> egui::Color32 {
    match kind {
        "bond" => egui::Color32::from_rgb(100, 180, 255),
        "organism" => egui::Color32::from_rgb(100, 255, 140),
        "reproduction" => egui::Color32::from_rgb(180, 255, 100),
        "death" => egui::Color32::from_rgb(255, 80, 80),
        "predation" => egui::Color32::from_rgb(255, 140, 60),
        "colony" => egui::Color32::from_rgb(255, 200, 80),
        "milestone" => egui::Color32::from_rgb(255, 255, 100),
        "culture" => egui::Color32::from_rgb(200, 140, 255),
        "metacog" => egui::Color32::from_rgb(140, 200, 255),
        "symbiogenesis" => egui::Color32::from_rgb(100, 255, 200),
        "sexual" => egui::Color32::from_rgb(255, 100, 150),
        _ => egui::Color32::from_rgb(180, 180, 180),
    }
}

/// Speed presets offered by the HUD.
const SPEED_PRESETS: &[(f32, &str)] = &[
    (1.0, "1×"),
    (5.0, "5×"),
    (10.0, "10×"),
    (20.0, "20×"),
];

// ──────────────────────────────────────────────────────────────────────────────
// Main UI system
// ──────────────────────────────────────────────────────────────────────────────

/// Grouped advanced era-detection resources to stay within Bevy's 16-param limit.
#[derive(SystemParam)]
struct UiEraParams<'w> {
    symbols: Res<'w, ActiveSymbolCodes>,
    genes: Res<'w, ActiveGeneCount>,
    metabolite: Res<'w, MetaboliteFlowRate>,
    memes: Res<'w, ActiveMemes>,
    metacog: Res<'w, MetaCogOrgCount>,
    build: Res<'w, BuildStructureCount>,
}

/// The single Bevy system that drives the entire UI each frame.
fn ui_system(
    mut contexts: EguiContexts,
    store: Res<ParticleStore>,
    mut config: ResMut<SimConfig>,
    stats: Res<SimStats>,
    history: Res<SimHistory>,
    events: Res<EventLog>,
    org_reg: Res<OrganismRegistry>,
    day_night: Res<DayNightState>,
    counters: Res<SimCounters>,
    mut ui_state: ResMut<UiState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    era_params: UiEraParams,
) {
    // ── Keyboard shortcuts ──────────────────────────────────────────────
    handle_keyboard(&keyboard, &mut config, &mut ui_state);

    let ctx = contexts.ctx_mut();

    // ── Responsive scaling for mobile ───────────────────────────────────
    // If the screen is small (e.g. phone), scale up the UI and use larger
    // touch targets.  We determine "mobile" by checking screen width.
    let screen_rect = ctx.input(|i| i.screen_rect());
    let is_mobile = screen_rect.width() < 800.0;
    if is_mobile {
        ctx.set_pixels_per_point(2.0);
    }

    ui_state.is_mobile = is_mobile;

    apply_dark_theme(ctx);

    // On mobile, auto-collapse panels on first frame to save space
    if is_mobile && stats.tick == 0 {
        ui_state.show_controls = false;
        ui_state.show_charts = false;
    }

    let era = detect_era(
        &stats, &counters, &store, &org_reg,
        &era_params.symbols, &era_params.genes, &era_params.metabolite,
        &era_params.memes, &era_params.metacog, &era_params.build,
    );
    let fps = 1.0 / time.delta_secs().max(1e-6);

    // ── 1. Top HUD bar ─────────────────────────────────────────────────
    if ui_state.show_hud {
        draw_hud_bar(ctx, &stats, &counters, &day_night, era, fps, &mut config);
    }

    // ── 2. Left panel: Controls ─────────────────────────────────────────
    if ui_state.show_controls {
        draw_controls_panel(ctx, &mut config, &stats, &mut ui_state);
    }

    // ── 3. Right panel: Inspector ───────────────────────────────────────
    if ui_state.show_inspector {
        draw_inspector_panel(ctx, &store, &org_reg, &mut ui_state);
    }

    // ── 4. Bottom panel: Mini-charts ────────────────────────────────────
    if ui_state.show_charts {
        draw_charts_panel(ctx, &history);
    }

    // ── 5. Event log (left-side, below controls) ────────────────────────
    if ui_state.show_events {
        draw_event_log(ctx, &events);
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
// Keyboard
// ──────────────────────────────────────────────────────────────────────────────

fn handle_keyboard(
    keyboard: &ButtonInput<KeyCode>,
    config: &mut SimConfig,
    ui_state: &mut UiState,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        config.paused = !config.paused;
    }
    if keyboard.just_pressed(KeyCode::Digit1) {
        config.speed = 1.0;
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        config.speed = 5.0;
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        config.speed = 10.0;
    }
    if keyboard.just_pressed(KeyCode::Digit4) {
        config.speed = 20.0;
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        ui_state.show_hud = !ui_state.show_hud;
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        ui_state.show_charts = !ui_state.show_charts;
    }
    if keyboard.just_pressed(KeyCode::KeyI) {
        ui_state.show_inspector = !ui_state.show_inspector;
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        ui_state.show_events = !ui_state.show_events;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        ui_state.reset_requested = true;
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// 1. Top HUD bar
// ──────────────────────────────────────────────────────────────────────────────

fn draw_hud_bar(
    ctx: &egui::Context,
    stats: &SimStats,
    _counters: &SimCounters,
    day_night: &DayNightState,
    era: &str,
    fps: f32,
    config: &mut SimConfig,
) {
    egui::TopBottomPanel::top("hud_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // ── Left: simulation info ───────────────────────────────
            ui.spacing_mut().item_spacing.x = 14.0;

            // Tick
            ui.colored_label(egui::Color32::from_rgb(120, 180, 255), "⏱");
            ui.label(format!("Tick {}", stats.tick));

            ui.separator();

            // Particles
            ui.colored_label(egui::Color32::from_rgb(180, 140, 255), "●");
            ui.label(format!("{}", stats.particle_count));

            ui.separator();

            // Organisms
            ui.colored_label(egui::Color32::from_rgb(100, 255, 140), "🧬");
            ui.label(format!("{}", stats.organism_count));

            ui.separator();

            // Bonds
            ui.colored_label(egui::Color32::from_rgb(100, 180, 255), "🔗");
            ui.label(format!("{}", stats.bond_count));

            ui.separator();

            // Colonies
            ui.colored_label(egui::Color32::from_rgb(255, 200, 80), "🏘️");
            ui.label(format!("{}", stats.colony_count));

            ui.separator();

            // Max generation
            ui.colored_label(egui::Color32::from_rgb(200, 140, 255), "Gen");
            ui.label(format!("{}", stats.max_generation));

            ui.separator();

            // Era
            ui.colored_label(egui::Color32::from_rgb(255, 220, 100), "Era");
            ui.label(era);

            ui.separator();

            // Day/Night phase
            let (phase_icon, phase_label) = if day_night.is_day {
                ("☀️", "Day")
            } else {
                ("🌙", "Night")
            };
            ui.label(phase_icon);
            ui.label(phase_label);

            // ── Right: FPS + speed controls ─────────────────────────
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Pause button
                let pause_text = if config.paused { "▶ Play" } else { "⏸ Pause" };
                if ui.button(pause_text).clicked() {
                    config.paused = !config.paused;
                }

                ui.separator();

                // Speed presets
                for &(spd, label) in SPEED_PRESETS.iter().rev() {
                    let is_active = (config.speed - spd).abs() < 0.5;
                    let btn = egui::Button::new(label);
                    let response = if is_active {
                        ui.add(btn.fill(egui::Color32::from_rgb(60, 90, 160)))
                    } else {
                        ui.add(btn)
                    };
                    if response.clicked() {
                        config.speed = spd;
                    }
                }

                ui.separator();

                // FPS
                ui.label(format!("FPS: {:.0}", fps));
            });
        });
    });
}

// ──────────────────────────────────────────────────────────────────────────────
// 2. Left panel: Controls
// ──────────────────────────────────────────────────────────────────────────────

fn draw_controls_panel(
    ctx: &egui::Context,
    config: &mut SimConfig,
    stats: &SimStats,
    ui_state: &mut UiState,
) {
    egui::SidePanel::left("controls_panel")
        .default_width(220.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("⚙ Controls");
            ui.separator();
            ui.add_space(4.0);

            // Speed slider
            ui.label("Simulation speed");
            ui.add(egui::Slider::new(&mut config.speed, 1.0..=20.0).text("×"));
            ui.add_space(6.0);

            // Pause / Play toggle
            let pause_label = if config.paused {
                "▶  Resume"
            } else {
                "⏸  Pause"
            };
            if ui.button(pause_label).clicked() {
                config.paused = !config.paused;
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // World info
            ui.colored_label(egui::Color32::from_rgb(140, 200, 255), "🌍 World");
            ui.add_space(2.0);
            ui.label(format!("Size: {:.0}³", config.world_size));
            ui.label(format!("Temperature: {:.2}", config.temperature));
            ui.label(format!("Solar strength: {:.2}", config.solar_strength));
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Mutation rate slider
            ui.colored_label(egui::Color32::from_rgb(200, 160, 255), "🧬 Genetics");
            ui.add_space(2.0);
            ui.label("Mutation rate");
            ui.add(
                egui::Slider::new(&mut config.mutation_rate, 0.0..=1.0)
                    .fixed_decimals(3)
                    .text("rate"),
            );
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Particle count
            ui.colored_label(egui::Color32::from_rgb(180, 180, 255), "● Particles");
            ui.add_space(2.0);
            ui.label(format!("Count: {}", stats.particle_count));
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

            // Panel toggles (convenience)
            ui.separator();
            ui.add_space(4.0);
            ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "Panels");
            ui.checkbox(&mut ui_state.show_hud, "HUD  (H)");
            ui.checkbox(&mut ui_state.show_charts, "Charts  (C)");
            ui.checkbox(&mut ui_state.show_inspector, "Inspector  (I)");
            ui.checkbox(&mut ui_state.show_events, "Events  (E)");
        });
}

// ──────────────────────────────────────────────────────────────────────────────
// 3. Right panel: Inspector
// ──────────────────────────────────────────────────────────────────────────────

fn draw_inspector_panel(
    ctx: &egui::Context,
    store: &ParticleStore,
    org_reg: &OrganismRegistry,
    ui_state: &mut UiState,
) {
    egui::SidePanel::right("inspector_panel")
        .default_width(280.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("🔍 Inspector");
            ui.separator();

            let Some(idx) = ui_state.selected_particle else {
                ui.label("Click a particle to inspect it.");
                return;
            };

            // Bounds check
            if idx >= store.len() {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 100, 100),
                    "Selected particle no longer exists.",
                );
                ui_state.selected_particle = None;
                return;
            }

            // ── Basic info ──────────────────────────────────────────
            let ptype = &store.ptype[idx];
            let energy = store.energy[idx];

            ui.colored_label(type_color(ptype), format!("● Particle #{idx}"));
            ui.add_space(2.0);
            ui.label(format!("Type: {:?}", ptype));
            ui.label(format!(
                "Position: ({:.1}, {:.1}, {:.1})",
                store.x[idx], store.y[idx], store.z[idx]
            ));
            ui.label(format!(
                "Velocity: ({:.2}, {:.2}, {:.2})",
                store.vx[idx], store.vy[idx], store.vz[idx]
            ));
            ui.add_space(4.0);

            // ── Energy bar ──────────────────────────────────────────
            let max_energy = 200.0_f32; // assumed max for display
            let frac = (energy / max_energy).clamp(0.0, 1.0);
            ui.label(format!("Energy: {:.1}", energy));
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 14.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(40, 40, 50));
            let bar_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * frac, rect.height()),
            );
            painter.rect_filled(bar_rect, 3.0, energy_color(frac));
            ui.add_space(6.0);

            // ── Extended properties ─────────────────────────────────
            ui.separator();
            ui.colored_label(egui::Color32::from_rgb(140, 200, 255), "Properties");
            ui.add_space(2.0);
            ui.label(format!("Signal:  {:.3}", store.signal[idx]));
            ui.label(format!("Memory:  {:.3}", store.memory[idx]));
            ui.label(format!("Phase:   {:.3}", store.phase[idx]));
            ui.add_space(4.0);

            // ── Bonds ───────────────────────────────────────────────
            let bond_count = store.bond_count(idx);
            ui.separator();
            ui.colored_label(egui::Color32::from_rgb(100, 180, 255), "Bonds");
            ui.label(format!("Count: {}", bond_count));
            if bond_count > 0 {
                let partners = store.bond_partners(idx);
                for &p in partners.iter() {
                    if p < store.len() {
                        ui.label(format!("  → #{p} ({:?})", store.ptype[p]));
                    }
                }
            }
            ui.add_space(4.0);

            // ── Organism info ───────────────────────────────────────
            if let Some(org_id) = store.organism_id_opt(idx) {
                ui.separator();
                ui.colored_label(egui::Color32::from_rgb(100, 255, 140), "🧬 Organism");
                ui.add_space(2.0);

                if let Some(org) = org_reg.get(org_id) {
                    ui.label(format!("ID: {}", org_id));
                    ui.label(format!("Size: {}", org.members.len()));
                    ui.label(format!("Generation: {}", org.generation));
                    ui.label(format!("Fitness: {:.2}", org.fitness));
                    ui.label(format!("Specialization: {:.2}", org.specialization));
                } else {
                    ui.label(format!("ID: {} (details unavailable)", org_id));
                }

                ui.add_space(2.0);
                ui.label(format!("Cell role: {:?}", store.cell_role[idx]));
            }
            ui.add_space(4.0);

            // ── Gene / epigenetic / culture / metacog ────────────────
            ui.separator();
            ui.colored_label(egui::Color32::from_rgb(200, 160, 255), "Advanced");
            ui.add_space(2.0);
            ui.label(format!("Gene expression: {:.3}", store.gene_expr[idx]));
            ui.label(format!("Epi weight:      {:.3}", store.epi_weight[idx]));
            ui.label(format!("Cultural meme:   {}", store.cultural_meme[idx]));
            ui.label(format!("Meta-cog level:  {:.3}", store.meta_cog_level[idx]));
        });
}

// ──────────────────────────────────────────────────────────────────────────────
// 4. Bottom panel: Mini-charts
// ──────────────────────────────────────────────────────────────────────────────

fn draw_charts_panel(ctx: &egui::Context, history: &SimHistory) {
    egui::TopBottomPanel::bottom("charts_panel")
        .default_height(110.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let organisms_data = history.organisms.to_vec();
                draw_mini_chart(
                    ui,
                    "Organisms",
                    &organisms_data,
                    egui::Color32::from_rgb(100, 255, 140),
                );
                ui.add_space(8.0);
                let bonds_data = history.bonds.to_vec();
                draw_mini_chart(
                    ui,
                    "Bonds",
                    &bonds_data,
                    egui::Color32::from_rgb(100, 180, 255),
                );
                ui.add_space(8.0);
                let energy_data = history.energy.to_vec();
                draw_mini_chart(
                    ui,
                    "Energy",
                    &energy_data,
                    egui::Color32::from_rgb(255, 220, 80),
                );
                ui.add_space(8.0);
                let generation_data = history.generation.to_vec();
                draw_mini_chart(
                    ui,
                    "Max Gen",
                    &generation_data,
                    egui::Color32::from_rgb(200, 140, 255),
                );
                ui.add_space(8.0);
                let colonies_data = history.colonies.to_vec();
                draw_mini_chart(
                    ui,
                    "Colonies",
                    &colonies_data,
                    egui::Color32::from_rgb(255, 200, 80),
                );
            });
        });
}

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

// ──────────────────────────────────────────────────────────────────────────────
// 5. Event Log
// ──────────────────────────────────────────────────────────────────────────────

fn draw_event_log(ctx: &egui::Context, events: &EventLog) {
    egui::Window::new("📜 Event Log")
        .default_pos(egui::pos2(8.0, 300.0))
        .default_width(260.0)
        .default_height(260.0)
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    // Show the last 50 events, oldest first.
                    let entries = events.recent(50);
                    for entry in entries.iter() {
                        let kind = entry.event_type.kind();
                        let emoji = event_emoji(kind);
                        let color = event_color(kind);
                        ui.colored_label(
                            color,
                            format!("[{}] {} {}", entry.tick, emoji, entry.text),
                        );
                    }

                    if entries.is_empty() {
                        ui.label("No events yet…");
                    }
                });
        });
}
