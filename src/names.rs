//! Names that the SDE does not store: stations, stargates, stars, and most
//! planets, moons and asteroid belts.

use crate::Sde;
use crate::ids::{self, IdKind, StationId, SystemId};
use crate::model::NpcStation;
use std::borrow::Cow;

/// 1 → `I`, 4 → `IV`. Numbers outside 1..=3999 come back as digits.
pub(crate) fn roman(n: u32) -> String {
    if !(1..=3999).contains(&n) {
        return n.to_string();
    }
    const PARTS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut n = n;
    let mut out = String::new();
    for (value, text) in PARTS {
        while n >= value {
            out.push_str(text);
            n -= value;
        }
    }
    out
}

/// A station's in-game name: the planet or moon it orbits, its owner and, if
/// `use_operation_name` is set, its operation, e.g.
/// `Jita IV - Moon 4 - Caldari Navy Assembly Plant`.
pub(crate) fn derive_station_name(sde: &Sde, station: &NpcStation) -> String {
    // A station that orbits a star, as in Zarzakh, starts with the system name.
    let place = sde
        .celestial_name(station.orbit_id)
        .filter(|_| !sde.stars.contains_key(&station.orbit_id))
        .unwrap_or_else(|| Cow::Borrowed(sde.system_name(station.solar_system_id)));
    let mut name = place.into_owned();
    if let Some(corp) = sde.npc_corporations.get(&station.owner_id) {
        name.push_str(" - ");
        name.push_str(&corp.name);
    }
    if station.use_operation_name
        && let Some(op) = sde.station_operations.get(&station.operation_id)
    {
        name.push(' ');
        name.push_str(&op.operation_name);
    }
    name
}

impl Sde {
    /// A planet's name, such as `Jita IV` or `Amarr VIII (Oris)`: the SDE's
    /// name if it has one, else the system name and the planet's numeral.
    fn planet_name(&self, id: u32, system_id: SystemId, celestial_index: u32) -> Cow<'_, str> {
        let unique = self.planets.get(&id).and_then(|p| p.unique_name.as_deref());
        unique_or(unique, || {
            format!("{} {}", self.system_name(system_id), roman(celestial_index))
        })
    }

    fn system_name(&self, id: SystemId) -> &str {
        self.solar_systems
            .get(&id)
            .map_or("Unknown system", |s| s.name.as_str())
    }

    /// The in-game name of anything with an ID in the SDE's location and NPC
    /// ranges: regions, constellations, systems, stars, planets, moons,
    /// asteroid belts, stargates, NPC stations, factions, NPC corporations and
    /// NPC characters. For types use [`Sde::type_name`]. Player IDs return
    /// `None`.
    pub fn name(&self, id: u32) -> Option<Cow<'_, str>> {
        fn borrowed(s: &str) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(s))
        }
        match ids::kind(id) {
            IdKind::Faction => borrowed(&self.factions.get(&id)?.name),
            IdKind::NpcCorporation => borrowed(&self.npc_corporations.get(&id)?.name),
            IdKind::NpcCharacter => borrowed(&self.npc_characters.get(&id)?.name),
            IdKind::Region => borrowed(&self.regions.get(&id)?.name),
            IdKind::Constellation => borrowed(&self.constellations.get(&id)?.name),
            IdKind::SolarSystem => borrowed(&self.solar_systems.get(&id)?.name),
            IdKind::Celestial => self.celestial_name(id),
            IdKind::Stargate => {
                let gate = self.stargates.get(&id)?;
                let to = self.system_name(gate.destination.solar_system_id);
                Some(Cow::Owned(format!("Stargate ({to})")))
            }
            IdKind::Station => borrowed(self.station_name(id)?),
            _ => None,
        }
    }

    /// The name of an NPC station, e.g.
    /// `Jita IV - Moon 4 - Caldari Navy Assembly Plant`.
    pub fn station_name(&self, id: StationId) -> Option<&str> {
        self.index.station_names.get(&id).map(String::as_str)
    }

    /// The name of a star, planet, moon or asteroid belt. Uses the SDE's name
    /// where it has one. Otherwise moons and belts are named after their
    /// planet, e.g. `Eon Prime - Moon 1`.
    pub fn celestial_name(&self, id: u32) -> Option<Cow<'_, str>> {
        if let Some(moon) = self.moons.get(&id) {
            return Some(unique_or(moon.unique_name.as_deref(), || {
                let planet =
                    self.planet_name(moon.orbit_id, moon.solar_system_id, moon.celestial_index);
                format!("{planet} - Moon {}", moon.orbit_index)
            }));
        }
        if let Some(planet) = self.planets.get(&id) {
            return Some(self.planet_name(id, planet.solar_system_id, planet.celestial_index));
        }
        if let Some(belt) = self.asteroid_belts.get(&id) {
            return Some(unique_or(belt.unique_name.as_deref(), || {
                let planet =
                    self.planet_name(belt.orbit_id, belt.solar_system_id, belt.celestial_index);
                format!("{planet} - Asteroid Belt {}", belt.orbit_index)
            }));
        }
        let star = self.stars.get(&id)?;
        Some(Cow::Owned(format!(
            "{} - Star",
            self.system_name(star.solar_system_id)
        )))
    }
}

/// The SDE's own name if it has one, else a derived name.
fn unique_or(unique: Option<&str>, derive: impl FnOnce() -> String) -> Cow<'_, str> {
    unique.map_or_else(|| Cow::Owned(derive()), Cow::Borrowed)
}

#[cfg(test)]
mod tests {
    use super::roman;

    #[test]
    fn roman_numerals() {
        assert_eq!(roman(1), "I");
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(9), "IX");
        assert_eq!(roman(18), "XVIII");
        assert_eq!(roman(1994), "MCMXCIV");
        assert_eq!(roman(0), "0");
        assert_eq!(roman(4000), "4000");
    }
}
