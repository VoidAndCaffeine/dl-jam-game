use crate::components::boss::{AttackKind, BossAttack};
use crate::components::effect_sprite::EffectSprite;
use crate::components::player::Player;
use crate::constants::HAZARD_FADE_FRACTION;
use crate::resources::effect_sprite::EffectSpriteAssets;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// Assets needed to draw an effect, all optional so headless tests run without
/// an asset server or atlas layout storage.
#[derive(SystemParam)]
pub struct EffectSpriteAccess<'w> {
    cache: ResMut<'w, EffectSpriteAssets>,
    server: Option<Res<'w, AssetServer>>,
    layouts: Option<ResMut<'w, Assets<TextureAtlasLayout>>>,
    images: Option<Res<'w, Assets<Image>>>,
}

/// Keeps each effect's sheet in sync with the phase of the attack it renders.
///
/// A falling attack telegraphs with a ground shadow and swaps to its impact art
/// the moment it arms; thrown attacks stay hidden until they launch.
pub fn sync_attack_effects(mut attacks: Query<(&BossAttack, &mut EffectSprite, &mut Sprite)>) {
    for (attack, mut effect, mut sprite) in attacks.iter_mut() {
        let desired = if attack.armed {
            attack.kind.effect()
        } else {
            attack.kind.windup_effect()
        };
        if effect.kind != desired {
            effect.play(desired, attack.phase);
        }
        effect.size = effect_size(attack);

        let alpha = effect_alpha(attack);
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
}

/// The world size an effect is drawn at.
///
/// The ground shadow marking a falling attack is scaled to that attack's own
/// footprint (the same disc the slam or debris hitbox uses); everything else
/// reads its natural size from the effect.
fn effect_size(attack: &BossAttack) -> Vec2 {
    if !attack.armed && matches!(attack.kind, AttackKind::Slam | AttackKind::Debris) {
        Vec2::splat(attack.radius * 2.0)
    } else {
        attack.kind.effect().world_size(attack.phase)
    }
}

/// How visible an effect is right now.
///
/// Thrown attacks (wave, spray, wisp) hide during their windup so the boss's
/// clip is the only tell. Lingering hazards fade as they expire.
fn effect_alpha(attack: &BossAttack) -> f32 {
    if !attack.armed && !attack.kind.shows_windup() {
        // Thrown attacks stay hidden until they launch.
        return 0.0;
    }
    if fades_away(attack.kind) && attack.total > 0.0 {
        let life = (attack.remaining / attack.total).clamp(0.0, 1.0);
        return HAZARD_FADE_FRACTION + (1.0 - HAZARD_FADE_FRACTION) * life;
    }
    1.0
}

/// Hazards that dwindle as their lifetime runs out. The amalgamation channel
/// keeps full opacity so its charge reads clearly right up to the blast.
fn fades_away(kind: AttackKind) -> bool {
    matches!(
        kind,
        AttackKind::SurgeTrail | AttackKind::AcidPool | AttackKind::MercuryPool
    )
}

/// Advances every effect's clip and pushes the matching sheet section onto the
/// sprite it renders.
pub fn animate_effect_sprites(
    time: Res<Time>,
    mut assets: EffectSpriteAccess,
    mut effects: Query<(&mut EffectSprite, &mut Sprite), (Without<Player>,)>,
) {
    let dt = time.delta_secs();
    for (mut effect, mut sprite) in effects.iter_mut() {
        effect.advance(dt);
        apply_effect_sprite(&mut assets, &effect, &mut sprite);
    }
}

fn apply_effect_sprite(
    assets: &mut EffectSpriteAccess,
    effect: &EffectSprite,
    sprite: &mut Sprite,
) {
    let Some(server) = assets.server.as_deref() else {
        return;
    };
    let Some(layouts) = assets.layouts.as_deref_mut() else {
        return;
    };

    let image = assets.cache.image_for(effect.kind, server);
    sprite.image = image.clone();
    sprite.custom_size = Some(effect.size * effect.kind.graphic_scale());

    if effect.kind.frames() <= 1 {
        sprite.texture_atlas = None;
        return;
    }

    // The sheets are all 5x5 grids; the frame size is read off the sheet so a
    // trimmed sheet (the quicksilver wave) gets the right layout automatically.
    let Some(images) = assets.images.as_deref() else {
        return;
    };
    let Some(texture) = images.get(&image) else {
        return;
    };
    let size = texture.size();
    if size.x == 0 || size.y == 0 {
        return;
    }
    let frame = UVec2::new(size.x / 5, size.y / 5);
    if frame.x == 0 || frame.y == 0 {
        return;
    }

    let layout = match assets.cache.layout_for(frame) {
        Some(handle) => handle.clone(),
        None => {
            let handle = layouts.add(EffectSpriteAssets::grid_layout(frame));
            assets.cache.set_layout(frame, handle.clone());
            handle
        }
    };
    sprite.texture_atlas = Some(TextureAtlas {
        layout,
        index: effect.atlas_index(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossAttack;
    use crate::components::effect_sprite::EffectKind;
    use crate::constants::SURGE_TRAIL_LIFE;

    fn attack(kind: AttackKind, armed: bool) -> BossAttack {
        let mut attack = BossAttack::new(kind, Vec2::ZERO);
        attack.armed = armed;
        attack
    }

    #[test]
    fn a_thrown_attack_is_hidden_until_it_arms() {
        assert_eq!(effect_alpha(&attack(AttackKind::Wave, false)), 0.0);
        assert_eq!(effect_alpha(&attack(AttackKind::Wave, true)), 1.0);
    }

    #[test]
    fn a_falling_attack_is_visible_while_it_winds_up() {
        assert!(effect_alpha(&attack(AttackKind::Debris, false)) > 0.0);
        assert!(effect_alpha(&attack(AttackKind::Slam, false)) > 0.0);
    }

    #[test]
    fn a_ground_hazard_fades_as_it_expires() {
        let mut fresh =
            BossAttack::new(AttackKind::AcidPool, Vec2::ZERO).with_lifetime(SURGE_TRAIL_LIFE);
        let bright = effect_alpha(&fresh);

        fresh.remaining = 0.0;
        let dim = effect_alpha(&fresh);
        assert!(dim < bright, "a dying hazard is dimmer: {dim} < {bright}");
        assert!(dim > 0.0, "it never fades to fully invisible");
    }

    #[test]
    fn a_plain_attack_is_fully_opaque() {
        assert_eq!(effect_alpha(&attack(AttackKind::Slam, true)), 1.0);
    }

    #[test]
    fn the_shadow_marker_matches_the_attacks_footprint() {
        let mut slam = BossAttack::new(AttackKind::Slam, Vec2::ZERO).with_radius(54.0);
        slam.armed = false;
        assert_eq!(effect_size(&slam), Vec2::splat(108.0));

        let mut debris = BossAttack::new(AttackKind::Debris, Vec2::ZERO).with_radius(20.0);
        debris.armed = false;
        assert_eq!(effect_size(&debris), Vec2::splat(40.0));
    }

    #[test]
    fn a_live_attack_draws_at_its_own_effect_size() {
        let slam = BossAttack::new(AttackKind::Slam, Vec2::ZERO);
        assert_eq!(
            effect_size(&slam),
            EffectKind::ExcavatorSlam.world_size(slam.phase)
        );
    }
}
