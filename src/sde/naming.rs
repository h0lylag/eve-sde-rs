//! The names of celestials and stations, from their records and the rules in
//! [`names`](crate::names). They work on any [`Source`], so the in-memory
//! SDE, a SQLite store and an import in progress all name things alike.

use super::Source;
use crate::error::Result;
use crate::ids::{CelestialId, SystemId};
use crate::model::{
    AsteroidBelt, Moon, NpcCorporation, NpcStation, Planet, SolarSystem, Star, StationOperation,
};
use crate::names;

/// What a system is called where its record is missing.
const UNKNOWN_SYSTEM: &str = "Unknown system";

/// A system's name.
pub(crate) fn system_name(source: &impl Source, id: SystemId) -> Result<String> {
    Ok(source
        .get_record::<SolarSystem>(&id)?
        .map_or_else(|| UNKNOWN_SYSTEM.to_owned(), |system| system.name))
}

/// A planet's name, such as `Jita IV` or `Amarr VIII (Oris)`: the SDE's name
/// if it has one, else the system and the planet's numeral.
fn planet_name(
    source: &impl Source,
    id: CelestialId,
    system: SystemId,
    celestial_index: u32,
) -> Result<String> {
    if let Some(name) = source
        .get_record::<Planet>(&id)?
        .and_then(|p| p.unique_name)
    {
        return Ok(name);
    }
    Ok(names::planet(
        &system_name(source, system)?,
        celestial_index,
    ))
}

/// The name of a star, planet, moon or asteroid belt. The SDE's name where it
/// has one; otherwise moons and belts are named after their planet, such as
/// `Eon Prime - Moon 1`.
pub(crate) fn celestial_name(source: &impl Source, id: CelestialId) -> Result<Option<String>> {
    if let Some(moon) = source.get_record::<Moon>(&id)? {
        if let Some(name) = moon.unique_name {
            return Ok(Some(name));
        }
        let planet = planet_name(
            source,
            moon.orbit_id,
            moon.solar_system_id,
            moon.celestial_index,
        )?;
        return Ok(Some(names::moon(&planet, moon.orbit_index)));
    }
    if let Some(planet) = source.get_record::<Planet>(&id)? {
        if let Some(name) = planet.unique_name {
            return Ok(Some(name));
        }
        let system = system_name(source, planet.solar_system_id)?;
        return Ok(Some(names::planet(&system, planet.celestial_index)));
    }
    if let Some(belt) = source.get_record::<AsteroidBelt>(&id)? {
        if let Some(name) = belt.unique_name {
            return Ok(Some(name));
        }
        let planet = planet_name(
            source,
            belt.orbit_id,
            belt.solar_system_id,
            belt.celestial_index,
        )?;
        return Ok(Some(names::asteroid_belt(&planet, belt.orbit_index)));
    }
    let Some(star) = source.get_record::<Star>(&id)? else {
        return Ok(None);
    };
    Ok(Some(names::star(&system_name(
        source,
        star.solar_system_id,
    )?)))
}

/// A station's in-game name, such as
/// `Jita IV - Moon 4 - Caldari Navy Assembly Plant`.
pub(crate) fn station_name(source: &impl Source, station: &NpcStation) -> Result<String> {
    // A station that orbits a star, as in Zarzakh, starts with the system's
    // name.
    let orbits_star = source.get_record::<Star>(&station.orbit_id)?.is_some();
    let place = match celestial_name(source, station.orbit_id)? {
        Some(name) if !orbits_star => name,
        _ => system_name(source, station.solar_system_id)?,
    };
    let owner = source
        .get_record::<NpcCorporation>(&station.owner_id)?
        .map(|corporation| corporation.name);
    let operation = if station.use_operation_name {
        source
            .get_record::<StationOperation>(&station.operation_id)?
            .map(|operation| operation.operation_name)
    } else {
        None
    };
    Ok(names::station(
        &place,
        owner.as_deref(),
        operation.as_deref(),
    ))
}
