//! Genesis 2.0 — Native Bevy application entry point

use bevy::prelude::*;
use genesis_sim::GenesisSimPlugin;

#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Parser, Debug)]
#[command(name = "genesis", version, about = "Genesis 2.0 — Open-Ended Evolution Simulator")]
struct Cli {
    #[arg(long)]
    headless: bool,
    #[arg(long, default_value_t = 1000)]
    ticks: u64,
    #[arg(long)]
    json: bool,
    #[arg(long, default_value_t = 0)]
    report_every: u64,
    #[arg(long)]
    speed: Option<f32>,
    #[arg(long, default_value_t = 0)]
    seed: u64,
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        run_gui();
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let cli = Cli::parse();
        if cli.headless {
            run_headless(&cli);
        } else {
            run_gui();
        }
    }
}

fn run_gui() {
    use genesis_render::GenesisRenderPlugin;
    use genesis_ui::GenesisUiPlugin;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Genesis 2.0 — Open-Ended Evolution".into(),
                resolution: (1200., 800.).into(),
                present_mode: bevy::window::PresentMode::AutoVsync,
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                canvas: Some("#bevy-canvas".to_string()),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GenesisSimPlugin)
        .add_plugins(GenesisRenderPlugin)
        .add_plugins(GenesisUiPlugin)
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
#[allow(dead_code)]
struct HeadlessConfig {
    target_ticks: u64,
    report_every: u64,
    json_output: bool,
    speed: f32,
    seed: u64,
    start_time: std::time::Instant,
}

#[cfg(not(target_arch = "wasm32"))]
fn run_headless(cli: &Cli) {
    use std::time::Duration;
    let speed = cli.speed.unwrap_or(200.0);
    let seed = cli.seed;
    if !cli.json {
        eprintln!("🧬 Genesis 2.0 — Headless Mode");
        eprintln!("   Ticks:  {}", cli.ticks);
        eprintln!("   Speed:  {}×", speed);
        if seed != 0 {
            eprintln!("   Seed:   {}", seed);
        }
        eprintln!();
    }
    App::new()
        .add_plugins(MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(Duration::ZERO)))
        .add_plugins(GenesisSimPlugin)
        .insert_resource(genesis_sim::resources::SimRng::new(if seed == 0 { rand::random() } else { seed }))
        .insert_resource(HeadlessConfig {
            target_ticks: cli.ticks,
            report_every: cli.report_every,
            json_output: cli.json,
            speed,
            seed,
            start_time: std::time::Instant::now(),
        })
        .add_systems(Update, headless_monitor)
        .run();
}

#[cfg(not(target_arch = "wasm32"))]
fn headless_monitor(
    store: Res<genesis_sim::particle_store::ParticleStore>,
    tick: Res<genesis_sim::resources::SimTick>,
    metrics_history: Res<genesis_sim::resources::MetricsHistory>,
    hcfg: Res<HeadlessConfig>,
    mut exit: EventWriter<AppExit>,
) {
    let current_tick = tick.0;
    if hcfg.report_every > 0 && current_tick > 0 && current_tick % hcfg.report_every == 0 {
        if !hcfg.json_output {
            eprintln!("  tick {:>6} | particles {:>5}", current_tick, store.population());
        }
    }
    if current_tick >= hcfg.target_ticks {
        let population = store.population();
        if hcfg.json_output {
            let elapsed = hcfg.start_time.elapsed().as_secs_f64();
            let tps = current_tick as f64 / elapsed;
            let output = serde_json::json!({
                "seed": hcfg.seed,
                "tick": current_tick,
                "elapsed_seconds": (elapsed * 100.0).round() / 100.0,
                "ticks_per_second": tps.round(),
                "particles": population,
                "snapshots": metrics_history.snapshots.len(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            eprintln!("\n═══════════════════════════════════════════════");
            eprintln!("  🧬 Simulation complete — tick {}", current_tick);
            eprintln!("  Particles: {:>6}", population);
            eprintln!("═══════════════════════════════════════════════");
        }
        exit.send(AppExit::Success);
    }
}
