//! The art that backs a growing crop: which sheet a pot shows for each growth
//! stage, and the component that drives its animation.
//!
//! Every plant sheet is the same 5x5 grid as the player, boss and effect packs
//! (25 frames of 256px), so one shared [`TextureAtlasLayout`] serves them all.
//! A crop only exists for the stages it has art for: the starter reed grows in a
//! single day, so it goes straight from seedling to grown.

use crate::components::player_sprite::{FRAME_COLUMNS, FRAME_ROWS, FRAME_SECONDS};
use crate::components::pot::CropType;
use bevy::prelude::*;

/// Frames in one plant sheet (a 5x5 grid).
pub const CROP_FRAME_COUNT: usize = (FRAME_COLUMNS * FRAME_ROWS) as usize;
/// How long one plant frame is shown.
pub const CROP_FRAME_SECONDS: f32 = FRAME_SECONDS;
/// The side length a crop is drawn at in world space.
pub const CROP_SPRITE_SIZE: f32 = 64.0;

/// One visual stage in a crop's life.
///
/// A crop only uses the stages its art ships. `GrowingLarge` is unique to the
/// tailings potato, the only plant with a fourth stage.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CropStage {
    Seedling,
    Growing,
    GrowingLarge,
    Grown,
}

/// The folder inside `assets/sprite_packs/Plants` that holds a stage's sheet.
///
/// The folders are named inconsistently (some Title Case, some lower snake
/// case), so the path is spelled out per crop rather than derived.
fn stage_folder(crop: CropType, stage: CropStage) -> &'static str {
    match (crop, stage) {
        (CropType::Starter, CropStage::Seedling) => "Quicksilver Reed Seedling",
        (CropType::Starter, CropStage::Grown) => "quicksilver_reed_grown-spritesheet",
        (CropType::CropA, CropStage::Seedling) => "Cinder Cap Seedling",
        (CropType::CropA, CropStage::Growing) => "cinder_cap_growing-spritesheet",
        (CropType::CropA, CropStage::Grown) => "cinder_cap_grown-spritesheet",
        (CropType::CropB, CropStage::Seedling) => "Tailings Potato Seedling",
        (CropType::CropB, CropStage::Growing) => "tailings_potato_growing-spritesheet",
        (CropType::CropB, CropStage::GrowingLarge) => "tailings_potato_growing_large-spritesheet",
        (CropType::CropB, CropStage::Grown) => "tailings_potato_grown-spritesheet",
        // A crop never asks for a stage its art does not have; fall back to the
        // seedling so a mistake is a wrong-looking plant rather than a panic.
        _ => "Quicksilver Reed Seedling",
    }
}

impl CropType {
    /// The stages this crop grows through, in order, from seedling to grown.
    ///
    /// The list length matches [`CropType::growth_days`] + 1, which is what
    /// [`CropType::stage_for_days_remaining`] relies on.
    pub fn stages(self) -> &'static [CropStage] {
        match self {
            CropType::Starter => &[CropStage::Seedling, CropStage::Grown],
            CropType::CropA => &[CropStage::Seedling, CropStage::Growing, CropStage::Grown],
            CropType::CropB => &[
                CropStage::Seedling,
                CropStage::Growing,
                CropStage::GrowingLarge,
                CropStage::Grown,
            ],
        }
    }

    /// The stage shown while `days_remaining` days are left before harvest.
    ///
    /// A pot starts at `growth_days` (its seedling) and counts down to 0 (grown),
    /// so the elapsed day is the index into [`CropType::stages`].
    pub fn stage_for_days_remaining(self, days_remaining: u8) -> CropStage {
        let stages = self.stages();
        let elapsed = self.growth_days().saturating_sub(days_remaining) as usize;
        stages[elapsed.min(stages.len() - 1)]
    }

    /// The full asset path of this crop's `stage` sheet.
    pub fn sheet_path(self, stage: CropStage) -> String {
        format!(
            "sprite_packs/Plants/{}/spritesheet.png",
            stage_folder(self, stage)
        )
    }

    /// Every sheet this crop needs, in stage order.
    pub fn sheet_paths(self) -> Vec<String> {
        self.stages()
            .iter()
            .map(|stage| self.sheet_path(*stage))
            .collect()
    }

    /// The scale this crop is drawn at, relative to [`CROP_SPRITE_SIZE`].
    ///
    /// The cinder cap art fills its frame more than the reed or the potato, so
    /// it is drawn smaller to read as the same size in the pot.
    pub fn sprite_scale(self) -> f32 {
        match self {
            CropType::CropA => 1.0 / 1.5,
            CropType::Starter | CropType::CropB => 1.0 / 1.25,
        }
    }
}

/// Draws a growing crop. Lives on a child of its pot, so despawning the pot
/// takes the plant with it.
#[derive(Component, Reflect, Debug)]
pub struct CropSprite {
    /// Which crop is growing.
    pub crop: CropType,
    /// The growth stage currently shown.
    pub stage: CropStage,
    /// True while the soil was watered today, so the plant reads as soaked.
    pub watered: bool,
    /// A per-plant phase offset so neighbouring plants of the same crop do not
    /// sway in lockstep.
    pub phase: usize,
    /// Frame within the 5x5 sheet.
    pub frame: usize,
    /// Drives frame advancement.
    pub frame_timer: Timer,
}

impl CropSprite {
    pub fn new(crop: CropType, stage: CropStage) -> Self {
        Self {
            crop,
            stage,
            watered: false,
            phase: 0,
            frame: 0,
            frame_timer: Timer::from_seconds(CROP_FRAME_SECONDS, TimerMode::Repeating),
        }
    }

    /// Sets the phase offset a plant starts at, so two plants of the same crop
    /// show different frames instead of moving identically.
    pub fn with_phase(mut self, phase: usize) -> Self {
        self.phase = phase % CROP_FRAME_COUNT;
        self
    }

    /// The sheet frame to draw, shifted by this plant's phase.
    pub fn atlas_frame(&self) -> usize {
        (self.frame + self.phase) % CROP_FRAME_COUNT
    }

    /// Switches to `stage` from its first frame, re-timing the clock. A no-op
    /// when the stage has not changed, so an animation in progress is not reset.
    pub fn set_stage(&mut self, stage: CropStage) {
        if self.stage == stage {
            return;
        }
        self.stage = stage;
        self.frame = 0;
        self.frame_timer = Timer::from_seconds(CROP_FRAME_SECONDS, TimerMode::Repeating);
    }

    /// Advances the looping clip by `dt`.
    pub fn advance(&mut self, dt: f32) {
        self.frame_timer
            .tick(core::time::Duration::from_secs_f32(dt));
        if self.frame_timer.just_finished() {
            self.frame = (self.frame + 1) % CROP_FRAME_COUNT;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_crop_starts_as_a_seedling_and_ends_grown() {
        for crop in CropType::ALL {
            let stages = crop.stages();
            assert_eq!(stages.first(), Some(&CropStage::Seedling), "{crop:?}");
            assert_eq!(stages.last(), Some(&CropStage::Grown), "{crop:?}");
            assert_eq!(
                stages.len(),
                crop.growth_days() as usize + 1,
                "{crop:?} needs one stage per growth day plus the seedling"
            );
        }
    }

    #[test]
    fn the_starter_reed_skips_the_middle_stages() {
        assert_eq!(
            CropType::Starter.stages(),
            &[CropStage::Seedling, CropStage::Grown]
        );
    }

    #[test]
    fn the_tailings_potato_has_four_stages() {
        assert_eq!(
            CropType::CropB.stages(),
            &[
                CropStage::Seedling,
                CropStage::Growing,
                CropStage::GrowingLarge,
                CropStage::Grown,
            ]
        );
    }

    #[test]
    fn the_stage_advances_as_the_days_count_down() {
        // Starter: planted at 1 day left, ready at 0.
        assert_eq!(
            CropType::Starter.stage_for_days_remaining(1),
            CropStage::Seedling
        );
        assert_eq!(
            CropType::Starter.stage_for_days_remaining(0),
            CropStage::Grown
        );

        // Crop A: 2, 1, 0.
        assert_eq!(
            CropType::CropA.stage_for_days_remaining(2),
            CropStage::Seedling
        );
        assert_eq!(
            CropType::CropA.stage_for_days_remaining(1),
            CropStage::Growing
        );
        assert_eq!(
            CropType::CropA.stage_for_days_remaining(0),
            CropStage::Grown
        );
    }

    #[test]
    fn the_potato_walks_all_four_stages() {
        let expected = [
            CropStage::Seedling,
            CropStage::Growing,
            CropStage::GrowingLarge,
            CropStage::Grown,
        ];
        for (days, stage) in (0..=3).rev().zip(expected) {
            assert_eq!(
                CropType::CropB.stage_for_days_remaining(days),
                stage,
                "with {days} days left"
            );
        }
    }

    #[test]
    fn an_out_of_range_day_still_maps_to_a_real_stage() {
        // Guards the clamp: a pot mid-day (days_remaining above growth_days) must
        // not index past the stage list.
        assert_eq!(
            CropType::Starter.stage_for_days_remaining(9),
            CropStage::Seedling
        );
    }

    #[test]
    fn every_stage_has_a_sheet_under_the_plants_folder() {
        for crop in CropType::ALL {
            let paths = crop.sheet_paths();
            assert_eq!(paths.len(), crop.stages().len());
            for path in paths {
                assert!(
                    path.starts_with("sprite_packs/Plants/") && path.ends_with("/spritesheet.png"),
                    "{crop:?} sheet path looks wrong: {path}"
                );
            }
        }
    }

    #[test]
    fn the_seedling_paths_match_the_picker_art() {
        assert_eq!(
            CropType::Starter.sheet_path(CropStage::Seedling),
            "sprite_packs/Plants/Quicksilver Reed Seedling/spritesheet.png"
        );
        assert_eq!(
            CropType::CropA.sheet_path(CropStage::Seedling),
            "sprite_packs/Plants/Cinder Cap Seedling/spritesheet.png"
        );
        assert_eq!(
            CropType::CropB.sheet_path(CropStage::Seedling),
            "sprite_packs/Plants/Tailings Potato Seedling/spritesheet.png"
        );
    }

    #[test]
    fn the_grown_paths_point_at_the_grown_sheets() {
        assert_eq!(
            CropType::Starter.sheet_path(CropStage::Grown),
            "sprite_packs/Plants/quicksilver_reed_grown-spritesheet/spritesheet.png"
        );
        assert_eq!(
            CropType::CropB.sheet_path(CropStage::GrowingLarge),
            "sprite_packs/Plants/tailings_potato_growing_large-spritesheet/spritesheet.png"
        );
    }

    #[test]
    fn the_mushroom_is_drawn_smaller_than_the_other_crops() {
        assert!(
            CropType::CropA.sprite_scale() < CropType::Starter.sprite_scale(),
            "the cinder cap fills more of its frame, so it shrinks further"
        );
        assert!((CropType::Starter.sprite_scale() - 0.8).abs() < 1e-6);
        assert!((CropType::CropB.sprite_scale() - 0.8).abs() < 1e-6);
        assert!((CropType::CropA.sprite_scale() - 1.0 / 1.5).abs() < 1e-6);
    }

    #[test]
    fn a_phase_offset_desyncs_the_displayed_frame() {
        let mut a = CropSprite::new(CropType::CropA, CropStage::Seedling).with_phase(0);
        let mut b = CropSprite::new(CropType::CropA, CropStage::Seedling).with_phase(5);
        a.frame = 3;
        b.frame = 3;
        assert_eq!(a.atlas_frame(), 3);
        assert_eq!(
            b.atlas_frame(),
            8,
            "the same progress shows a different frame"
        );
    }

    #[test]
    fn the_displayed_frame_wraps_with_the_phase() {
        let mut sprite = CropSprite::new(CropType::CropA, CropStage::Seedling).with_phase(3);
        sprite.frame = CROP_FRAME_COUNT - 1;
        assert_eq!(sprite.atlas_frame(), 2);
    }

    #[test]
    fn setting_the_same_stage_keeps_the_animation_going() {
        let mut sprite = CropSprite::new(CropType::CropB, CropStage::Growing);
        sprite.frame = 7;
        sprite.set_stage(CropStage::Growing);
        assert_eq!(sprite.frame, 7, "the clip must not restart on a no-op");
        sprite.set_stage(CropStage::GrowingLarge);
        assert_eq!(sprite.frame, 0, "a real stage change restarts the clip");
    }

    #[test]
    fn the_frame_loops_forever() {
        let mut sprite = CropSprite::new(CropType::CropA, CropStage::Seedling);
        for _ in 0..CROP_FRAME_COUNT {
            sprite.advance(CROP_FRAME_SECONDS + 0.001);
        }
        assert_eq!(sprite.frame, 0);
    }
}
