pub mod interaction;
pub mod farm;

pub use interaction::InteractionPlugin;
pub use interaction::{Interactable, FarmPot, CraftingStation, BossArenaEntry, NPC};
pub use farm::FarmPlugin;