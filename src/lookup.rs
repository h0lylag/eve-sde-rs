//! Name indexes, item hierarchy and dogma joins.

use crate::Sde;
use crate::ids::*;
use crate::model::*;
use std::collections::HashMap;
use std::hash::Hash;

type NameIndex = HashMap<String, Vec<u32>>;

fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Group values by key. Each list is sorted, without repeats.
fn multimap<K: Hash + Eq, V: Ord>(pairs: impl Iterator<Item = (K, V)>) -> HashMap<K, Vec<V>> {
    let mut map: HashMap<K, Vec<V>> = HashMap::new();
    for (key, value) in pairs {
        map.entry(key).or_default().push(value);
    }
    for values in map.values_mut() {
        values.sort_unstable();
        values.dedup();
    }
    map
}

/// Lowercased name → IDs. IDs whose `preferred` flag is set come first, then
/// lower IDs before higher.
fn name_index<'a>(rows: impl Iterator<Item = (u32, &'a str, bool)>) -> NameIndex {
    multimap(rows.map(|(id, name, preferred)| (key(name), (!preferred, id))))
        .into_iter()
        .map(|(name, ids)| (name, ids.into_iter().map(|(_, id)| id).collect()))
        .collect()
}

/// Everything derived from the tables at load time.
#[derive(Default)]
pub(crate) struct Index {
    types: NameIndex,
    groups: NameIndex,
    categories: NameIndex,
    market_groups: NameIndex,
    systems: NameIndex,
    constellations: NameIndex,
    regions: NameIndex,
    stations: NameIndex,
    factions: NameIndex,
    corporations: NameIndex,
    attributes: NameIndex,
    effects: NameIndex,
    pub(crate) station_names: HashMap<StationId, String>,
    pub(crate) neighbors: HashMap<SystemId, Vec<SystemId>>,
    types_by_group: HashMap<GroupId, Vec<TypeId>>,
    groups_by_category: HashMap<CategoryId, Vec<GroupId>>,
}

impl Index {
    pub(crate) fn build(sde: &Sde) -> Index {
        let station_names: HashMap<StationId, String> = sde
            .npc_stations
            .values()
            .map(|s| (s.id, crate::names::derive_station_name(sde, s)))
            .collect();

        Index {
            types: name_index(
                sde.types
                    .values()
                    .map(|t| (t.id, t.name.as_str(), t.published)),
            ),
            groups: name_index(
                sde.groups
                    .values()
                    .map(|g| (g.id, g.name.as_str(), g.published)),
            ),
            categories: name_index(
                sde.categories
                    .values()
                    .map(|c| (c.id, c.name.as_str(), c.published)),
            ),
            market_groups: name_index(
                sde.market_groups
                    .values()
                    .map(|g| (g.id, g.name.as_str(), true)),
            ),
            systems: name_index(
                sde.solar_systems
                    .values()
                    .map(|s| (s.id, s.name.as_str(), true)),
            ),
            constellations: name_index(
                sde.constellations
                    .values()
                    .map(|c| (c.id, c.name.as_str(), true)),
            ),
            regions: name_index(sde.regions.values().map(|r| (r.id, r.name.as_str(), true))),
            stations: name_index(station_names.iter().map(|(id, n)| (*id, n.as_str(), true))),
            factions: name_index(sde.factions.values().map(|f| (f.id, f.name.as_str(), true))),
            corporations: name_index(
                sde.npc_corporations
                    .values()
                    .map(|c| (c.id, c.name.as_str(), !c.deleted)),
            ),
            attributes: name_index(
                sde.dogma_attributes
                    .values()
                    .map(|a| (a.id, a.name.as_str(), true)),
            ),
            effects: name_index(
                sde.dogma_effects
                    .values()
                    .map(|e| (e.id, e.name.as_str(), true)),
            ),
            station_names,
            neighbors: multimap(
                sde.stargates
                    .values()
                    .map(|g| (g.solar_system_id, g.destination.solar_system_id)),
            ),
            types_by_group: multimap(sde.types.values().map(|t| (t.group_id, t.id))),
            groups_by_category: multimap(sde.groups.values().map(|g| (g.category_id, g.id))),
        }
    }
}

macro_rules! name_lookups {
    ($($one:ident, $all:ident, $field:ident, $id:ty, $what:literal;)*) => {
        impl Sde {$(
            #[doc = concat!("ID of the ", $what, " with this name, ignoring case. With several matches this is the first of [`Sde::", stringify!($all), "`].")]
            pub fn $one(&self, name: &str) -> Option<$id> {
                self.$all(name).first().copied()
            }

            #[doc = concat!("Every ", $what, " ID with this name, ignoring case. Published (for corporations: not deleted) IDs come first, then lower IDs before higher.")]
            pub fn $all(&self, name: &str) -> &[$id] {
                self.index.$field.get(&key(name)).map_or(&[], Vec::as_slice)
            }
        )*}
    };
}

name_lookups! {
    type_id, type_ids, types, TypeId, "type";
    group_id, group_ids, groups, GroupId, "group";
    category_id, category_ids, categories, CategoryId, "category";
    market_group_id, market_group_ids, market_groups, MarketGroupId, "market group";
    system_id, system_ids, systems, SystemId, "solar system";
    constellation_id, constellation_ids, constellations, ConstellationId, "constellation";
    region_id, region_ids, regions, RegionId, "region";
    station_id, station_ids, stations, StationId, "NPC station";
    faction_id, faction_ids, factions, FactionId, "faction";
    corporation_id, corporation_ids, corporations, CorporationId, "NPC corporation";
    attribute_id, attribute_ids, attributes, AttributeId, "dogma attribute (by internal name)";
    effect_id, effect_ids, effects, EffectId, "dogma effect (by internal name)";
}

/// The kind of fitting slot a module needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Slot {
    High,
    Mid,
    Low,
    Rig,
    Subsystem,
    Service,
}

/// Whether a module needs a turret or launcher hardpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Hardpoint {
    Turret,
    Launcher,
}

/// The effect that puts a module in each kind of slot.
const SLOT_EFFECTS: [(EffectId, Slot); 6] = [
    (12, Slot::High),        // hiPower
    (13, Slot::Mid),         // medPower
    (11, Slot::Low),         // loPower
    (2663, Slot::Rig),       // rigSlot
    (3772, Slot::Subsystem), // subSystem
    (6306, Slot::Service),   // serviceSlot
];

/// The effect that makes a module take each kind of hardpoint.
const HARDPOINT_EFFECTS: [(EffectId, Hardpoint); 2] = [
    (42, Hardpoint::Turret),   // turretFitted
    (40, Hardpoint::Launcher), // launcherFitted
];

impl Sde {
    /// English name of a type.
    pub fn type_name(&self, id: TypeId) -> Option<&str> {
        self.types.get(&id).map(|t| t.name.as_str())
    }

    pub fn group_of(&self, type_id: TypeId) -> Option<&Group> {
        self.groups.get(&self.types.get(&type_id)?.group_id)
    }

    pub fn category_of(&self, type_id: TypeId) -> Option<&Category> {
        self.categories.get(&self.group_of(type_id)?.category_id)
    }

    pub fn meta_group_of(&self, type_id: TypeId) -> Option<&MetaGroup> {
        self.meta_groups
            .get(&self.types.get(&type_id)?.meta_group_id?)
    }

    /// The market folders above a type, outermost first. Empty if the type is
    /// not sold on the market.
    pub fn market_path(&self, type_id: TypeId) -> Vec<&MarketGroup> {
        let mut path: Vec<&MarketGroup> = Vec::new();
        let mut next = self.types.get(&type_id).and_then(|t| t.market_group_id);
        while let Some(group) = next.and_then(|id| self.market_groups.get(&id)) {
            if path.iter().any(|seen| seen.id == group.id) {
                break; // The parents loop; stop rather than spin.
            }
            path.push(group);
            next = group.parent_group_id;
        }
        path.reverse();
        path
    }

    /// Type IDs in a group, ascending.
    pub fn types_in_group(&self, group_id: GroupId) -> &[TypeId] {
        self.index
            .types_by_group
            .get(&group_id)
            .map_or(&[], Vec::as_slice)
    }

    /// Group IDs in a category, ascending.
    pub fn groups_in_category(&self, category_id: CategoryId) -> &[GroupId] {
        self.index
            .groups_by_category
            .get(&category_id)
            .map_or(&[], Vec::as_slice)
    }

    /// A dogma attribute of a type. If the type does not list it, the
    /// attribute's default applies. `None` for an unknown type or attribute.
    pub fn attribute(&self, type_id: TypeId, attribute_id: AttributeId) -> Option<f64> {
        let listed = self.type_dogma.get(&type_id).and_then(|d| {
            d.dogma_attributes
                .iter()
                .find(|a| a.attribute_id == attribute_id)
        });
        match listed {
            Some(a) => Some(a.value),
            None if self.types.contains_key(&type_id) => self
                .dogma_attributes
                .get(&attribute_id)
                .map(|a| a.default_value),
            None => None,
        }
    }

    /// Like [`Sde::attribute`], by internal name such as `hiSlots` or `cpuOutput`.
    pub fn attribute_named(&self, type_id: TypeId, name: &str) -> Option<f64> {
        self.attribute(type_id, self.attribute_id(name)?)
    }

    /// The attributes a type lists, with their values.
    pub fn attributes(&self, type_id: TypeId) -> impl Iterator<Item = (&Attribute, f64)> {
        self.type_dogma
            .get(&type_id)
            .into_iter()
            .flat_map(|d| d.dogma_attributes.iter())
            .filter_map(|a| Some((self.dogma_attributes.get(&a.attribute_id)?, a.value)))
    }

    /// The effects a type has, and whether each is its default effect.
    pub fn effects(&self, type_id: TypeId) -> impl Iterator<Item = (&Effect, bool)> {
        self.type_dogma
            .get(&type_id)
            .into_iter()
            .flat_map(|d| d.dogma_effects.iter())
            .filter_map(|e| Some((self.dogma_effects.get(&e.effect_id)?, e.is_default)))
    }

    /// The slot a module is fitted in, from its slot effect.
    pub fn slot(&self, type_id: TypeId) -> Option<Slot> {
        self.first_effect(type_id, &SLOT_EFFECTS)
    }

    /// The hardpoint a module uses, if any.
    pub fn hardpoint(&self, type_id: TypeId) -> Option<Hardpoint> {
        self.first_effect(type_id, &HARDPOINT_EFFECTS)
    }

    /// The value next to the first effect in `table` that the type has.
    fn first_effect<T: Copy>(&self, type_id: TypeId, table: &[(EffectId, T)]) -> Option<T> {
        let effects = &self.type_dogma.get(&type_id)?.dogma_effects;
        table
            .iter()
            .find(|(effect_id, _)| effects.iter().any(|e| e.effect_id == *effect_id))
            .map(|&(_, value)| value)
    }
}
