//! Save / Load — JSON serialisation matching the web version's format.
//!
//! The web version (`sim.ts`) uses `JSON.stringify` / `JSON.parse` with a
//! well-defined schema (version 6).  We mirror that schema here so save
//! files are **fully cross-compatible** between native and web builds.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::components::{CellRole, ParticleType};
use crate::particle_store::ParticleStore;
use crate::resources::{PhylogenyTree, SimCounters};

// ─────────────────────────────────────────────────────────────────────────────
// Serialisable snapshot types
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level save-file structure (matches web `serialize()` output).
#[derive(Serialize, Deserialize)]
pub struct SaveState {
    pub version: u32,
    pub tick: u64,
    #[serde(rename = "totalRepro")]
    pub total_repro: u64,
    #[serde(rename = "totalPred")]
    pub total_pred: u64,
    #[serde(rename = "totalSexualRepro")]
    pub total_sexual_repro: u64,
    #[serde(rename = "totalSymbiogenesis")]
    pub total_symbiogenesis: u64,
    pub milestones: Vec<String>,
    pub particles: Vec<SaveParticle>,
    pub phylogeny: Vec<(u32, SavePhyloNode)>,
}

/// Per-particle data (matches the web `particles` array entries).
#[derive(Serialize, Deserialize)]
pub struct SaveParticle {
    pub id: u32,
    #[serde(rename = "type")]
    pub ptype: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    pub energy: f32,
    pub signal: f32,
    pub memory: f32,
    pub phase: f32,
    pub age: u32,
    pub bonds: Vec<u32>,
    pub alive: bool,
    #[serde(rename = "organismId")]
    pub organism_id: i32,
    #[serde(rename = "isDeposit")]
    pub is_deposit: bool,
    #[serde(rename = "comboBonus")]
    pub combo_bonus: f32,
    #[serde(rename = "geneExpr")]
    pub gene_expr: f32,
    #[serde(rename = "cellRole")]
    pub cell_role: u8,
    #[serde(rename = "epiWeight")]
    pub epi_weight: f32,
    #[serde(rename = "symbolCode")]
    pub symbol_code: u8,
    #[serde(rename = "heldTool")]
    pub held_tool: i32,
    #[serde(rename = "culturalMeme")]
    pub cultural_meme: u16,
    #[serde(rename = "metaCogLevel")]
    pub meta_cog_level: f32,
    pub signature: u32,
}

/// Phylogeny node (matches the web `phylogeny` map entries).
#[derive(Serialize, Deserialize)]
pub struct SavePhyloNode {
    pub parent_id: i32,
    pub tick: u64,
    pub generation: u32,
    pub size: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Serialize (snapshot → JSON string)
// ─────────────────────────────────────────────────────────────────────────────

/// Capture the current simulation state as a JSON string.
///
/// This is the Rust equivalent of the web `Simulation.serialize()` method.
pub fn serialize_state(
    store: &ParticleStore,
    counters: &SimCounters,
    phylogeny: &PhylogenyTree,
) -> String {
    let particles: Vec<SaveParticle> = (0..store.len())
        .filter(|&i| store.alive[i])
        .map(|i| SaveParticle {
            id: store.id[i],
            ptype: store.ptype[i].as_index() as u8,
            x: store.x[i],
            y: store.y[i],
            z: store.z[i],
            vx: store.vx[i],
            vy: store.vy[i],
            vz: store.vz[i],
            energy: store.energy[i],
            signal: store.signal[i],
            memory: store.memory[i],
            phase: store.phase[i],
            age: store.age[i],
            bonds: store.bonds[i].iter().copied().collect(),
            alive: true,
            organism_id: store.organism_id[i],
            is_deposit: store.is_deposit[i],
            combo_bonus: store.combo_bonus[i],
            gene_expr: store.gene_expr[i],
            cell_role: store.cell_role[i].as_index() as u8,
            epi_weight: store.epi_weight[i],
            symbol_code: store.symbol_code[i],
            held_tool: store.held_tool[i],
            cultural_meme: store.cultural_meme[i],
            meta_cog_level: store.meta_cog_level[i],
            signature: store.signature[i],
        })
        .collect();

    let phylo_entries: Vec<(u32, SavePhyloNode)> = phylogeny
        .nodes
        .iter()
        .map(|(&id, node)| {
            (
                id,
                SavePhyloNode {
                    parent_id: node.parent_id,
                    tick: node.tick,
                    generation: node.generation,
                    size: node.size,
                },
            )
        })
        .collect();

    let state = SaveState {
        version: 6,
        tick: counters.tick,
        total_repro: counters.total_repro,
        total_pred: counters.total_pred,
        total_sexual_repro: counters.total_sexual_repro,
        total_symbiogenesis: counters.total_symbiogenesis,
        milestones: counters.milestones.iter().cloned().collect(),
        particles,
        phylogeny: phylo_entries,
    };

    serde_json::to_string(&state).unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// Deserialize (JSON string → restore state)
// ─────────────────────────────────────────────────────────────────────────────

/// Load a previously saved state from a JSON string.
///
/// This is the Rust equivalent of the web `Simulation.loadState(json)` method.
/// It **replaces** the current particle store and counters entirely.
pub fn deserialize_state(
    json: &str,
    store: &mut ParticleStore,
    counters: &mut SimCounters,
    phylogeny: &mut PhylogenyTree,
) -> Result<(), String> {
    let data: SaveState =
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;

    // ── Clear current state ───────────────────────────────────────────────
    *store = ParticleStore::default();
    let mut max_id: u32 = 0;

    // ── Restore particles ─────────────────────────────────────────────────
    store.reserve(data.particles.len());
    for p in &data.particles {
        let idx = store.id.len();

        store.id.push(p.id);
        store.ptype.push(
            ParticleType::try_from_index(p.ptype as usize)
                .unwrap_or(ParticleType::Alpha),
        );
        store.alive.push(p.alive);
        store.is_deposit.push(p.is_deposit);
        store.x.push(p.x);
        store.y.push(p.y);
        store.z.push(p.z);
        store.vx.push(p.vx);
        store.vy.push(p.vy);
        store.vz.push(p.vz);
        store.energy.push(p.energy);
        store.signal.push(p.signal);
        store.memory.push(p.memory);
        store.phase.push(p.phase);
        store.age.push(p.age);
        store.bonds.push(p.bonds.iter().copied().collect::<HashSet<u32>>());
        store.organism_id.push(p.organism_id);
        store.combo_bonus.push(p.combo_bonus);
        store.gene_expr.push(p.gene_expr);
        store.cell_role.push(CellRole::from_index(p.cell_role as usize));
        store.epi_weight.push(p.epi_weight);
        store.symbol_code.push(p.symbol_code);
        store.held_tool.push(p.held_tool);
        store.cultural_meme.push(p.cultural_meme);
        store.meta_cog_level.push(p.meta_cog_level);
        store.signature.push(p.signature);

        store.id_to_index.insert(p.id, idx);
        if p.id > max_id {
            max_id = p.id;
        }
    }
    store.next_id = max_id + 1;
    store.rebuild_index();

    // ── Restore counters ──────────────────────────────────────────────────
    counters.tick = data.tick;
    counters.total_repro = data.total_repro;
    counters.total_pred = data.total_pred;
    counters.total_sexual_repro = data.total_sexual_repro;
    counters.total_symbiogenesis = data.total_symbiogenesis;
    counters.milestones = data.milestones.into_iter().collect();

    // ── Restore phylogeny ─────────────────────────────────────────────────
    phylogeny.nodes.clear();
    for (id, node) in data.phylogeny {
        phylogeny.nodes.insert(
            id,
            crate::resources::PhyloNode {
                parent_id: node.parent_id,
                tick: node.tick,
                generation: node.generation,
                size: node.size,
            },
        );
    }

    Ok(())
}
