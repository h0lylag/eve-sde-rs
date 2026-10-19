//! The lookups of [`Sde`]: by name, along the item hierarchy, into dogma,
//! across the map, and the names of anything with an ID.

use super::{Named, Relation, Sde, Source, naming};
use crate::error::Result;
use crate::fitting::{Hardpoint, Slot};
use crate::ids::{
    self, AttributeId, CategoryId, CelestialId, ConstellationId, CorporationId, EffectId,
    FactionId, GroupId, IdKind, MarketGroupId, RegionId, StargateId, StationId, SystemId, TypeId,
};
use crate::model::{
    Attribute, Category, Constellation, Effect, Faction, Group, MarketGroup, MetaGroup,
    NpcCharacter, NpcCorporation, NpcStation, Region, SolarSystem, Stargate, Type, TypeDogma,
};
use crate::names;

macro_rules! name_lookups {
    ($($one:ident, $all:ident, $named:ident, $id:ident, $what:literal, $order:literal $(, $note:literal)?;)*) => {
        impl Sde {$(
            #[doc = concat!(
                "The ID of the ", $what, " with this name, ignoring case and surrounding whitespace.",
                $(" ", $note,)?
                " With several matches this is the first of [`Sde::", stringify!($all), "`].",
                "\n\n# Errors\n\nSee [`Sde`](Sde#errors)."
            )]
            pub fn $one(&self, name: &str) -> Result<Option<$id>> {
                Ok(self.$all(name)?.first().copied())
            }

            #[doc = concat!(
                "Every ", $what, " ID with this name, ignoring case and surrounding whitespace.",
                $(" ", $note,)? " ", $order,
                "\n\n# Errors\n\nSee [`Sde`](Sde#errors)."
            )]
            pub fn $all(&self, name: &str) -> Result<Vec<$id>> {
                Ok(self.ids_named(Named::$named, name)?.into_iter().map($id).collect())
            }
        )*}
    };
}

name_lookups! {
    type_id, type_ids, Type, TypeId, "type",
        "Published types come first, then lower IDs.";
    group_id, group_ids, Group, GroupId, "group",
        "Published groups come first, then lower IDs.";
    category_id, category_ids, Category, CategoryId, "category",
        "Published categories come first, then lower IDs.";
    market_group_id, market_group_ids, MarketGroup, MarketGroupId, "market group",
        "Lower IDs come first.";
    system_id, system_ids, System, SystemId, "solar system",
        "Lower IDs come first.";
    constellation_id, constellation_ids, Constellation, ConstellationId, "constellation",
        "Lower IDs come first.";
    region_id, region_ids, Region, RegionId, "region",
        "Lower IDs come first.";
    station_id, station_ids, Station, StationId, "NPC station",
        "Lower IDs come first.", "Stations have the names the game shows, such as \
        `Jita IV - Moon 4 - Caldari Navy Assembly Plant`.";
    faction_id, faction_ids, Faction, FactionId, "faction",
        "Lower IDs come first.";
    corporation_id, corporation_ids, Corporation, CorporationId, "NPC corporation",
        "Corporations that are not deleted come first, then lower IDs.";
    attribute_id, attribute_ids, Attribute, AttributeId, "dogma attribute",
        "Lower IDs come first.", "Use the internal name, such as `hiSlots`.";
    effect_id, effect_ids, Effect, EffectId, "dogma effect",
        "Lower IDs come first.", "Use the internal name, such as `hiPower`.";
}

impl Sde {
    /// The English name of a type.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn type_name(&self, id: TypeId) -> Result<Option<String>> {
        Ok(self.get_record::<Type>(&id)?.map(|ty| ty.name))
    }

    /// The group of a type, such as Frigate for the Rifter.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn group_of(&self, type_id: TypeId) -> Result<Option<Group>> {
        let Some(ty) = self.get_record::<Type>(&type_id)? else {
            return Ok(None);
        };
        self.get_record(&ty.group_id)
    }

    /// The category of a type, such as Ship for the Rifter.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn category_of(&self, type_id: TypeId) -> Result<Option<Category>> {
        let Some(group) = self.group_of(type_id)? else {
            return Ok(None);
        };
        self.get_record(&group.category_id)
    }

    /// The meta group of a type, such as Tech II. `None` if the type has no
    /// meta group.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn meta_group_of(&self, type_id: TypeId) -> Result<Option<MetaGroup>> {
        let Some(id) = self
            .get_record::<Type>(&type_id)?
            .and_then(|ty| ty.meta_group_id)
        else {
            return Ok(None);
        };
        self.get_record(&id)
    }

    /// The market folders above a type, outermost first. Empty if the type is
    /// not sold on the market.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn market_path(&self, type_id: TypeId) -> Result<Vec<MarketGroup>> {
        let mut path: Vec<MarketGroup> = Vec::new();
        let mut next = self
            .get_record::<Type>(&type_id)?
            .and_then(|ty| ty.market_group_id);
        while let Some(id) = next {
            if path.iter().any(|seen| seen.id == id) {
                break; // The parents form a loop.
            }
            let Some(group) = self.get_record::<MarketGroup>(&id)? else {
                break;
            };
            next = group.parent_group_id;
            path.push(group);
        }
        path.reverse();
        Ok(path)
    }

    /// The types in a group, ascending.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn types_in_group(&self, group_id: GroupId) -> Result<Vec<TypeId>> {
        let ids = self.children(Relation::TypesInGroup, group_id.0)?;
        Ok(ids.into_iter().map(TypeId).collect())
    }

    /// The groups in a category, ascending.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn groups_in_category(&self, category_id: CategoryId) -> Result<Vec<GroupId>> {
        let ids = self.children(Relation::GroupsInCategory, category_id.0)?;
        Ok(ids.into_iter().map(GroupId).collect())
    }

    /// The value of a dogma attribute for a type. If the type does not list
    /// the attribute, the attribute's default value applies. `None` for an
    /// unknown type or attribute.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn attribute(&self, type_id: TypeId, attribute_id: AttributeId) -> Result<Option<f64>> {
        let listed = self.get_record::<TypeDogma>(&type_id)?.and_then(|dogma| {
            dogma
                .dogma_attributes
                .into_iter()
                .find(|a| a.attribute_id == attribute_id)
        });
        if let Some(attribute) = listed {
            return Ok(Some(attribute.value));
        }
        if self.get_record::<Type>(&type_id)?.is_none() {
            return Ok(None);
        }
        let attribute = self.get_record::<Attribute>(&attribute_id)?;
        Ok(attribute.map(|a| a.default_value))
    }

    /// Like [`Sde::attribute`], by internal name such as `hiSlots` or
    /// `cpuOutput`.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn attribute_named(&self, type_id: TypeId, name: &str) -> Result<Option<f64>> {
        match self.attribute_id(name)? {
            Some(attribute_id) => self.attribute(type_id, attribute_id),
            None => Ok(None),
        }
    }

    /// The attributes a type lists, with their values.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn attributes(&self, type_id: TypeId) -> Result<Vec<(Attribute, f64)>> {
        let Some(dogma) = self.get_record::<TypeDogma>(&type_id)? else {
            return Ok(Vec::new());
        };
        let mut attributes = Vec::new();
        for listed in dogma.dogma_attributes {
            if let Some(attribute) = self.get_record::<Attribute>(&listed.attribute_id)? {
                attributes.push((attribute, listed.value));
            }
        }
        Ok(attributes)
    }

    /// The effects a type has, and whether each is its default effect.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn effects(&self, type_id: TypeId) -> Result<Vec<(Effect, bool)>> {
        let Some(dogma) = self.get_record::<TypeDogma>(&type_id)? else {
            return Ok(Vec::new());
        };
        let mut effects = Vec::new();
        for listed in dogma.dogma_effects {
            if let Some(effect) = self.get_record::<Effect>(&listed.effect_id)? {
                effects.push((effect, listed.is_default));
            }
        }
        Ok(effects)
    }

    /// The slot a module is fitted in, from its slot effect.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn slot(&self, type_id: TypeId) -> Result<Option<Slot>> {
        let dogma = self.get_record::<TypeDogma>(&type_id)?;
        Ok(dogma.and_then(|dogma| dogma.slot()))
    }

    /// The hardpoint a module uses, if any.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn hardpoint(&self, type_id: TypeId) -> Result<Option<Hardpoint>> {
        let dogma = self.get_record::<TypeDogma>(&type_id)?;
        Ok(dogma.and_then(|dogma| dogma.hardpoint()))
    }

    /// The systems one stargate away, ascending. Empty for a system without
    /// stargates, such as a wormhole system.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn neighbors(&self, system_id: SystemId) -> Result<Vec<SystemId>> {
        let mut systems: Vec<SystemId> = self
            .gates(system_id)?
            .into_iter()
            .map(|gate| gate.destination.solar_system_id)
            .collect();
        systems.sort_unstable();
        systems.dedup();
        Ok(systems)
    }

    /// The stargates in a system.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn gates(&self, system_id: SystemId) -> Result<Vec<Stargate>> {
        let Some(system) = self.get_record::<SolarSystem>(&system_id)? else {
            return Ok(Vec::new());
        };
        let mut gates = Vec::new();
        for id in &system.stargate_ids {
            if let Some(gate) = self.get_record::<Stargate>(id)? {
                gates.push(gate);
            }
        }
        Ok(gates)
    }

    /// The in-game name of anything with an ID in the SDE's location and NPC
    /// ranges: regions, constellations, systems, stars, planets, moons,
    /// asteroid belts, stargates, NPC stations, factions, NPC corporations and
    /// NPC characters. For types use [`Sde::type_name`]. Player IDs give
    /// `None`.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn name(&self, id: u32) -> Result<Option<String>> {
        Ok(match ids::kind(id) {
            IdKind::Faction => self.get_record::<Faction>(&FactionId(id))?.map(|f| f.name),
            IdKind::NpcCorporation => self
                .get_record::<NpcCorporation>(&CorporationId(id))?
                .map(|c| c.name),
            IdKind::NpcCharacter => self.get_record::<NpcCharacter>(&id)?.map(|c| c.name),
            IdKind::Region => self.get_record::<Region>(&RegionId(id))?.map(|r| r.name),
            IdKind::Constellation => self
                .get_record::<Constellation>(&ConstellationId(id))?
                .map(|c| c.name),
            IdKind::SolarSystem => self
                .get_record::<SolarSystem>(&SystemId(id))?
                .map(|s| s.name),
            IdKind::Celestial => self.celestial_name(CelestialId(id))?,
            IdKind::Stargate => match self.get_record::<Stargate>(&StargateId(id))? {
                Some(gate) => {
                    let to = naming::system_name(self, gate.destination.solar_system_id)?;
                    Some(names::stargate(&to))
                }
                None => None,
            },
            IdKind::Station => self.station_name(StationId(id))?,
            IdKind::Various | IdKind::Player | IdKind::Other => None,
        })
    }

    /// The name of an NPC station, such as
    /// `Jita IV - Moon 4 - Caldari Navy Assembly Plant`.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn station_name(&self, id: StationId) -> Result<Option<String>> {
        match self.get_record::<NpcStation>(&id)? {
            Some(station) => Ok(Some(naming::station_name(self, &station)?)),
            None => Ok(None),
        }
    }

    /// The name of a star, planet, moon or asteroid belt. The SDE's name where
    /// it has one; otherwise moons and belts are named after their planet,
    /// such as `Eon Prime - Moon 1`.
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn celestial_name(&self, id: CelestialId) -> Result<Option<String>> {
        naming::celestial_name(self, id)
    }
}
