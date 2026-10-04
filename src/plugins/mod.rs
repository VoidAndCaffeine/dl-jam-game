pub mod interaction;
pub mod farm;

pub use interaction::InteractionPlugin;
pub use interaction::{Interactable, FarmPot, CraftingStation, BossArenaEntry, NPC, HighlightMarker};
pub use farm::FarmPlugin;