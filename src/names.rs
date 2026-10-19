//! The in-game names that the SDE does not store: most planets, moons and
//! asteroid belts, and every star, stargate and NPC station.
//!
//! Each function builds one name from the names it is made of. Where the SDE
//! gives a planet, moon or belt a `unique_name`, use that instead.

/// The Roman numeral of `n`, as in planet names: 1 is `I`, 4 is `IV`.
/// Numbers outside 1..=3999 come back as digits.
///
/// # Examples
///
/// ```
/// assert_eq!(eve_sde::names::roman(4), "IV");
/// assert_eq!(eve_sde::names::roman(0), "0");
/// ```
#[must_use]
pub fn roman(n: u32) -> String {
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

    if !(1..=3999).contains(&n) {
        return n.to_string();
    }
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

/// A planet's name: its system and the planet's `celestial_index` as a Roman
/// numeral.
///
/// # Examples
///
/// ```
/// assert_eq!(eve_sde::names::planet("Jita", 4), "Jita IV");
/// ```
#[must_use]
pub fn planet(system: &str, celestial_index: u32) -> String {
    format!("{system} {}", roman(celestial_index))
}

/// A moon's name: the name of the planet it orbits and the moon's
/// `orbit_index`.
///
/// # Examples
///
/// ```
/// assert_eq!(eve_sde::names::moon("Jita IV", 4), "Jita IV - Moon 4");
/// ```
#[must_use]
pub fn moon(planet: &str, orbit_index: u32) -> String {
    format!("{planet} - Moon {orbit_index}")
}

/// An asteroid belt's name: the name of the planet it orbits and the belt's
/// `orbit_index`.
///
/// # Examples
///
/// ```
/// assert_eq!(
///     eve_sde::names::asteroid_belt("Jita IV", 1),
///     "Jita IV - Asteroid Belt 1"
/// );
/// ```
#[must_use]
pub fn asteroid_belt(planet: &str, orbit_index: u32) -> String {
    format!("{planet} - Asteroid Belt {orbit_index}")
}

/// A star's name: its system's name.
///
/// # Examples
///
/// ```
/// assert_eq!(eve_sde::names::star("Jita"), "Jita - Star");
/// ```
#[must_use]
pub fn star(system: &str) -> String {
    format!("{system} - Star")
}

/// A stargate's name: the name of the system it leads to.
///
/// # Examples
///
/// ```
/// assert_eq!(eve_sde::names::stargate("Perimeter"), "Stargate (Perimeter)");
/// ```
#[must_use]
pub fn stargate(destination: &str) -> String {
    format!("Stargate ({destination})")
}

/// An NPC station's name.
///
/// - `place` is the name of the planet or moon the station orbits. A station
///   that orbits its system's star, as in Zarzakh, uses the system's name.
/// - `owner` is the name of the corporation that owns it.
/// - `operation` is the station operation's name, such as `Assembly Plant`.
///   Pass it only when the station's `use_operation_name` is set.
///
/// # Examples
///
/// ```
/// let name = eve_sde::names::station(
///     "Jita IV - Moon 4",
///     Some("Caldari Navy"),
///     Some("Assembly Plant"),
/// );
/// assert_eq!(name, "Jita IV - Moon 4 - Caldari Navy Assembly Plant");
/// ```
#[must_use]
pub fn station(place: &str, owner: Option<&str>, operation: Option<&str>) -> String {
    let mut name = place.to_owned();
    if let Some(owner) = owner {
        name.push_str(" - ");
        name.push_str(owner);
    }
    if let Some(operation) = operation {
        name.push(' ');
        name.push_str(operation);
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roman_numerals_cover_the_usual_range_and_fall_back_to_digits() {
        assert_eq!(roman(1), "I");
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(9), "IX");
        assert_eq!(roman(18), "XVIII");
        assert_eq!(roman(1994), "MCMXCIV");
        assert_eq!(roman(0), "0");
        assert_eq!(roman(4000), "4000");
    }

    #[test]
    fn stations_name_their_owner_and_operation_only_when_given() {
        assert_eq!(station("Alpha IV", None, None), "Alpha IV");
        assert_eq!(
            station("Alpha IV", Some("Caldari Navy"), None),
            "Alpha IV - Caldari Navy"
        );
    }
}
