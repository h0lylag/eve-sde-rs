//! What sort of space a solar system is in, and its security.

use crate::model::SolarSystem;

/// What sort of space a system is in, from its ID range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpaceKind {
    /// The gated universe, including Pochven, Zarzakh and the three Jove
    /// regions that no gate reaches.
    KnownSpace,
    /// Wormhole space, including Thera.
    Wormhole,
    /// Abyssal deadspace.
    Abyssal,
    /// Systems in any other ID range, such as the empty systems of the VR
    /// regions.
    Special,
}

/// High, low or null security, as the game groups systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SecurityBand {
    /// Shown as 0.5 to 1.0.
    High,
    /// Shown as 0.1 to 0.4.
    Low,
    /// Shown as 0.0 or below.
    Null,
}

impl SolarSystem {
    /// What sort of space this system is in, from its ID.
    #[must_use]
    pub fn space(&self) -> SpaceKind {
        match self.id.0 {
            30_000_000..=30_999_999 => SpaceKind::KnownSpace,
            31_000_000..=31_999_999 => SpaceKind::Wormhole,
            32_000_000..=32_999_999 => SpaceKind::Abyssal,
            _ => SpaceKind::Special,
        }
    }

    /// The security status, rounded the way the game shows it. Anything above
    /// zero but below 0.05 shows as 0.1.
    #[must_use]
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
    #[must_use]
    pub fn security_display(&self) -> String {
        format!("{:.1}", self.security_rounded())
    }

    /// The security band: high from 0.45 (shown as 0.5), low above 0.0, null
    /// otherwise.
    #[must_use]
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
