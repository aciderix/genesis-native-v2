//! Simulation systems for the Genesis Engine.
//!
//! Each module implements one or more Bevy systems that operate on the
//! centralised `ParticleStore` resource. Systems are intended to run in
//! this order each tick:
//!
//! 1. **Grid** — Rebuild spatial hash grid from current positions
//! 2. **Forces** — Compute and apply all forces to velocities
//! 3. **Integrate** — Euler-integrate velocities into positions, apply drag
//! 4. **Bonds** — Form new bonds and break existing ones
//! 5. **Organisms** — Detect connected components, assign cell roles
//! 6. **Metabolism** — Energy economy: solar, chemosynthesis, sharing, death
//! 7. **Signals** — Neural signal propagation and phase oscillation
//! 8. **Reproduction** — Organism binary fission
//! 9. **Colonies** — Detect multi-organism colonies
//! 10. **Fields** — Diffuse, decay, and propagate all scalar fields
//! 11. **Advanced** — Combos, gene regulation, epigenetics, cell roles
//! 12. **Symbols & Tools** — Symbol communication, tool use, construction
//! 13. **Culture & Metacog** — Cultural memes, meta-cognition
//! 14. **V6 Systems** — Immune, symbiogenesis, sexual reproduction, niches

pub mod grid;
pub mod forces;
pub mod integrate;
pub mod bonds;
pub mod organisms;
pub mod metabolism;
pub mod signals;
pub mod reproduction;
pub mod colonies;
pub mod fields;
pub mod advanced;
pub mod symbols_tools;
pub mod culture_metacog;
pub mod v6_systems;

// Re-export all system functions for convenient registration
pub use grid::rebuild_grid_system;
pub use forces::apply_forces_system;
pub use integrate::integrate_system;
pub use bonds::{form_bonds_system, break_bonds_system};
pub use organisms::detect_organisms_system;
pub use metabolism::metabolism_system;
pub use signals::{propagate_signals_system, update_phase_system};
pub use reproduction::reproduce_system;
pub use colonies::detect_colonies_system;
pub use fields::update_fields_system;

// Advanced systems (P2.1, P2.4, P2.5, P3.1, P3.2)
pub use advanced::advanced_systems;
pub use advanced::{combos_system, gene_regulation_system, epigenetics_system, cell_roles_system};

// Symbolic & tool systems (P3.4, P4.1, P4.2)
pub use symbols_tools::{symbols_system, tool_use_system, construction_system};

// Culture & meta-cognition systems (P4.3, P4.4)
pub use culture_metacog::{culture_system, meta_cognition_system};

// V6 systems (V6.2, V6.3, V6.5, M3)
pub use v6_systems::{immune_system, symbiogenesis_system, sexual_reproduce_system, niche_bonuses_system};

// Re-export inner functions for simulation_tick
pub use grid::rebuild_grid_inner;
pub use forces::apply_forces_inner;
pub use integrate::integrate_inner;
pub use bonds::{form_bonds_inner, break_bonds_inner};
pub use organisms::detect_organisms_inner;
pub use metabolism::metabolism_inner;
pub use signals::{propagate_signals_inner, update_phase_inner};
pub use reproduction::reproduce_inner;
pub use colonies::detect_colonies_inner;
pub use fields::update_fields_inner;
pub use advanced::advanced_systems_inner;
pub use symbols_tools::{symbols_inner, tool_use_inner, construction_inner};
pub use culture_metacog::{culture_inner, meta_cognition_inner};
pub use v6_systems::{immune_inner, symbiogenesis_inner, sexual_reproduce_inner, niche_bonuses_inner};
