//! ID aliases and what an ID range means.
//!
//! The aliases are documentation only: they are all `u32`, so the compiler
//! will not stop you mixing a `TypeId` with a `SystemId`.

pub type TypeId = u32;
pub type GroupId = u32;
pub type CategoryId = u32;
pub type MarketGroupId = u32;
pub type MetaGroupId = u32;
pub type AttributeId = u32;
pub type EffectId = u32;
pub type RegionId = u32;
pub type ConstellationId = u32;
pub type SystemId = u32;
pub type StargateId = u32;
pub type StationId = u32;
pub type FactionId = u32;
pub type CorporationId = u32;

/// What kind of thing an ID refers to, from its numeric range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdKind {
    /// Types, groups, categories and other small IDs share this range.
    Various,
    Faction,
    NpcCorporation,
    NpcCharacter,
    Region,
    Constellation,
    SolarSystem,
    /// Star, planet, moon or asteroid belt.
    Celestial,
    Stargate,
    Station,
    /// Player characters, corporations, alliances and structures. The SDE does
    /// not know these; ask ESI.
    Player,
    Other,
}

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
}
