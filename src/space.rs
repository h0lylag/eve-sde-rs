//! Solar systems, security status and gate links.

use crate::Sde;
use crate::ids::SystemId;
use crate::model::{SolarSystem, Stargate};

/// What sort of space a system is in, from its ID range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpaceKind {
    /// The gated universe, including Pochven, Zarzakh and the systems that no
    /// gate reaches.
    KnownSpace,
    Wormhole,
    Abyssal,
    /// Event and test systems.
    Special,
}

/// High, low or null security, as the game groups systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecurityBand {
    High,
    Low,
    Null,
}

impl SolarSystem {
    /// What sort of space this system is in.
    pub fn space(&self) -> SpaceKind {
        match self.id {
            30_000_000..=30_999_999 => SpaceKind::KnownSpace,
            31_000_000..=31_999_999 => SpaceKind::Wormhole,
            32_000_000..=32_999_999 => SpaceKind::Abyssal,
            _ => SpaceKind::Special,
        }
    }

    /// Security status rounded the way the game shows it. Anything above zero
    /// but below 0.05 shows as 0.1.
    pub fn security_rounded(&self) -> f64 {
        let s = self.security_status;
        if s <= 0.0 || s >= 0.05 {
            // Adding 0.0 turns -0.0 into 0.0.
            (s * 10.0).round() / 10.0 + 0.0
        } else {
            0.1
        }
    }

    /// The security status as shown in game, e.g. `0.9` or `-0.3`.
    pub fn security_display(&self) -> String {
        format!("{:.1}", self.security_rounded())
    }

    /// High from 0.45 up (shown as 0.5), low above 0.0, null otherwise.
    pub fn security_band(&self) -> SecurityBand {
        if self.security_status >= 0.45 {
            SecurityBand::High
        } else if self.security_status > 0.0 {
            SecurityBand::Low
        } else {
            SecurityBand::Null
        }
    }
}

impl Sde {
    /// Systems reachable through one stargate, ascending. Empty for systems
    /// without gates (wormholes, abyssal space, a few isolated systems).
    pub fn neighbors(&self, system_id: SystemId) -> &[SystemId] {
        self.index
            .neighbors
            .get(&system_id)
            .map_or(&[], Vec::as_slice)
    }

    /// The stargates in a system.
    pub fn gates(&self, system_id: SystemId) -> impl Iterator<Item = &Stargate> {
        self.solar_systems
            .get(&system_id)
            .into_iter()
            .flat_map(|s| s.stargate_ids.iter())
            .filter_map(|id| self.stargates.get(id))
    }
}
