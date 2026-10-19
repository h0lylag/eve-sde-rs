//! ID types and what an ID range means.
//!
//! Each kind of ID the SDE shares between tables has its own type, so the
//! compiler stops a system ID from being passed where a type ID is expected.
//! The value is public: `TypeId(587)` makes one, and `id.0` reads it.
//!
//! Other IDs, such as icon or graphic IDs, are plain `u32`.

use std::fmt;
use std::num::TryFromIntError;

macro_rules! ids {
    ($($(#[$doc:meta])* $name:ident;)*) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "archive", derive(serde::Deserialize, serde::Serialize))]
        #[cfg_attr(feature = "archive", serde(transparent))]
        pub struct $name(pub u32);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        /// Every ID type can key a table.
        #[cfg(feature = "archive")]
        impl crate::tables::Key for $name {
            const KIND: crate::tables::KeyKind = crate::tables::KeyKind::Integer;
        }

        #[cfg(feature = "archive")]
        impl crate::tables::sealed::SealedKey for $name {
            #[cfg(feature = "sqlite")]
            fn sql_key(&self) -> rusqlite::types::ToSqlOutput<'_> {
                crate::tables::sealed::SealedKey::sql_key(&self.0)
            }
        }

        impl From<u32> for $name {
            fn from(id: u32) -> Self {
                Self(id)
            }
        }

        impl From<$name> for u32 {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        /// For databases and clients that store IDs as signed 64-bit numbers.
        impl From<$name> for i64 {
            fn from(id: $name) -> Self {
                i64::from(id.0)
            }
        }

        impl From<$name> for u64 {
            fn from(id: $name) -> Self {
                u64::from(id.0)
            }
        }

        /// Fails for a negative ID or one above `u32::MAX`, such as the `-1`
        /// that game clients use for "none".
        impl TryFrom<i64> for $name {
            type Error = TryFromIntError;

            fn try_from(id: i64) -> Result<Self, Self::Error> {
                u32::try_from(id).map(Self)
            }
        }

        /// Fails for an ID above `u32::MAX`.
        impl TryFrom<u64> for $name {
            type Error = TryFromIntError;

            fn try_from(id: u64) -> Result<Self, Self::Error> {
                u32::try_from(id).map(Self)
            }
        }
    )*};
}

ids! {
    /// Any item type: a ship, module, skill, blueprint and so on.
    TypeId;
    /// A group of types, such as Frigate.
    GroupId;
    /// A category of groups, such as Ship.
    CategoryId;
    /// A folder in the market tree.
    MarketGroupId;
    /// A meta group, such as Tech II or Faction.
    MetaGroupId;
    /// A dogma attribute, such as `hiSlots`.
    AttributeId;
    /// A dogma effect, such as `hiPower`.
    EffectId;
    /// A region, such as The Forge.
    RegionId;
    /// A constellation, such as Kimotoro.
    ConstellationId;
    /// A solar system, such as Jita.
    SystemId;
    /// A star, planet, moon, asteroid belt or secondary sun. They share one
    /// type because they share one range, and an `orbit_id` can point at a
    /// star, a planet or a moon.
    CelestialId;
    /// One end of a stargate pair.
    StargateId;
    /// An NPC station. Player structures have IDs too large for `u32`.
    StationId;
    /// A faction, such as the Caldari State.
    FactionId;
    /// An NPC corporation. The SDE has no player corporations.
    CorporationId;
}

/// What kind of thing an ID refers to, from its numeric range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IdKind {
    /// Types, groups, categories and other small IDs share this range.
    Various,
    /// A faction.
    Faction,
    /// An NPC corporation.
    NpcCorporation,
    /// Agents, corporation CEOs and other NPC characters.
    NpcCharacter,
    /// A region.
    Region,
    /// A constellation.
    Constellation,
    /// A solar system, including wormhole and Abyssal systems.
    SolarSystem,
    /// A star, planet, moon, asteroid belt or secondary sun.
    Celestial,
    /// One end of a stargate pair.
    Stargate,
    /// An NPC station.
    Station,
    /// Player characters, corporations and alliances. The SDE does not know
    /// these, so ask ESI. Structures and other items have IDs too large for
    /// `u32`.
    Player,
    /// A range this crate does not classify.
    Other,
}

/// What kind of thing `id` refers to, judging by its range alone. It does
/// not check that the ID exists.
#[must_use]
pub fn kind(id: u32) -> IdKind {
    match id {
        0..=499_999 => IdKind::Various,
        500_000..=599_999 => IdKind::Faction,
        1_000_000..=1_999_999 => IdKind::NpcCorporation,
        3_000_000..=3_999_999 => IdKind::NpcCharacter,
        10_000_000..=19_999_999 => IdKind::Region,
        20_000_000..=29_999_999 => IdKind::Constellation,
        30_000_000..=39_999_999 => IdKind::SolarSystem,
        40_000_000..=49_999_999 => IdKind::Celestial,
        50_000_000..=59_999_999 => IdKind::Stargate,
        60_000_000..=69_999_999 => IdKind::Station,
        90_000_000..=u32::MAX => IdKind::Player,
        _ => IdKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(kind(34), IdKind::Various);
        assert_eq!(kind(500_001), IdKind::Faction);
        assert_eq!(kind(1_000_035), IdKind::NpcCorporation);
        assert_eq!(kind(10_000_002), IdKind::Region);
        assert_eq!(kind(30_000_142), IdKind::SolarSystem);
        assert_eq!(kind(60_003_760), IdKind::Station);
        assert_eq!(kind(98_000_001), IdKind::Player);
        assert_eq!(kind(80_000_001), IdKind::Other);
    }

    #[test]
    fn ids_convert_from_wider_integers_only_when_they_fit() {
        assert_eq!(TypeId::try_from(587_i64), Ok(TypeId(587)));
        assert!(TypeId::try_from(-1_i64).is_err());
        assert!(SystemId::try_from(u64::from(u32::MAX) + 1).is_err());
        assert_eq!(u32::from(SystemId(30_000_142)), 30_000_142);
        assert_eq!(i64::from(TypeId(587)), 587);
        assert_eq!(TypeId(587).to_string(), "587");
    }

    #[cfg(feature = "archive")]
    #[test]
    fn ids_read_and_write_as_plain_numbers() {
        let id: TypeId = serde_json::from_str("587").unwrap();
        assert_eq!(id, TypeId(587));
        assert_eq!(serde_json::to_string(&id).unwrap(), "587");
    }
}
