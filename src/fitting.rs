//! Which slot and hardpoint a module needs, from its dogma effects.

use crate::ids::EffectId;

/// The kind of fitting slot a module needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Slot {
    /// A high slot (effect `hiPower`).
    High,
    /// A medium slot (effect `medPower`).
    Mid,
    /// A low slot (effect `loPower`).
    Low,
    /// A rig slot (effect `rigSlot`).
    Rig,
    /// A Tech III cruiser subsystem slot (effect `subSystem`).
    Subsystem,
    /// A service slot of an Upwell structure (effect `serviceSlot`).
    Service,
}

/// Whether a module needs a turret or launcher hardpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Hardpoint {
    /// A turret hardpoint (effect `turretFitted`).
    Turret,
    /// A launcher hardpoint (effect `launcherFitted`).
    Launcher,
}

/// The effect that puts a module in each kind of slot, in order of priority.
const SLOT_EFFECTS: [(EffectId, Slot); 6] = [
    (EffectId(12), Slot::High),
    (EffectId(13), Slot::Mid),
    (EffectId(11), Slot::Low),
    (EffectId(2663), Slot::Rig),
    (EffectId(3772), Slot::Subsystem),
    (EffectId(6306), Slot::Service),
];

/// The effect that makes a module take each kind of hardpoint, in order of
/// priority.
const HARDPOINT_EFFECTS: [(EffectId, Hardpoint); 2] = [
    (EffectId(42), Hardpoint::Turret),
    (EffectId(40), Hardpoint::Launcher),
];

impl Slot {
    /// The slot that a type with these effects is fitted in, or `None` if
    /// none of them is a slot effect.
    ///
    /// # Examples
    ///
    /// ```
    /// use eve_sde::Slot;
    /// use eve_sde::ids::EffectId;
    ///
    /// // A Damage Control has `loPower` (11) among its effects.
    /// assert_eq!(Slot::from_effects([EffectId(16), EffectId(11)]), Some(Slot::Low));
    /// ```
    pub fn from_effects(effects: impl IntoIterator<Item = EffectId>) -> Option<Self> {
        first_match(effects, &SLOT_EFFECTS)
    }
}

impl Hardpoint {
    /// The hardpoint that a type with these effects uses, or `None` if none
    /// of them is a hardpoint effect.
    pub fn from_effects(effects: impl IntoIterator<Item = EffectId>) -> Option<Self> {
        first_match(effects, &HARDPOINT_EFFECTS)
    }
}

/// The value of the highest-priority entry of `table` whose effect is among
/// `effects`.
fn first_match<T: Copy>(
    effects: impl IntoIterator<Item = EffectId>,
    table: &[(EffectId, T)],
) -> Option<T> {
    effects
        .into_iter()
        .filter_map(|effect| table.iter().position(|(id, _)| *id == effect))
        .min()
        .map(|i| table[i].1)
}

#[cfg(feature = "archive")]
impl crate::model::TypeDogma {
    /// The slot this type is fitted in, from its effects.
    #[must_use]
    pub fn slot(&self) -> Option<Slot> {
        Slot::from_effects(self.dogma_effects.iter().map(|e| e.effect_id))
    }

    /// The hardpoint this type uses, from its effects.
    #[must_use]
    pub fn hardpoint(&self) -> Option<Hardpoint> {
        Hardpoint::from_effects(self.dogma_effects.iter().map(|e| e.effect_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modules_get_their_slot_and_hardpoint_from_their_effects() {
        let effects = [EffectId(12), EffectId(42)];
        assert_eq!(Slot::from_effects(effects), Some(Slot::High));
        assert_eq!(Hardpoint::from_effects(effects), Some(Hardpoint::Turret));
        assert_eq!(Slot::from_effects([EffectId(1)]), None);
        assert_eq!(Hardpoint::from_effects([]), None);
    }

    #[test]
    fn the_earlier_slot_effect_wins_whatever_the_order_of_effects() {
        assert_eq!(
            Slot::from_effects([EffectId(11), EffectId(12)]),
            Some(Slot::High)
        );
    }
}
