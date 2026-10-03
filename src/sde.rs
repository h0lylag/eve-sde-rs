use crate::archive::{Archive, BuildInfo, Record};
use crate::error::Result;
use crate::lookup::Index;
use crate::model::*;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

macro_rules! tables {
    ($($field:ident: $ty:ident),* $(,)?) => {
        /// The whole SDE in memory, English text only.
        ///
        /// Each table has a read-only accessor, such as [`Sde::types`].
        /// Tables and their lookup indexes stay fixed after loading.
        ///
        /// ```compile_fail
        /// let mut sde = eve_sde::Sde::load("sde.zip").unwrap();
        /// sde.types().get_mut(&34).unwrap().name = "Changed".into();
        /// ```
        ///
        /// ```compile_fail
        /// let mut sde = eve_sde::Sde::load("sde.zip").unwrap();
        /// sde.stargates.clear();
        /// ```
        pub struct Sde {
            build: BuildInfo,
            unmodeled: Vec<String>,
            pub(crate) index: Index,
            $(
                pub(crate) $field: HashMap<<$ty as Record>::Id, $ty>,
            )*
        }

        /// File names of every table this crate reads.
        pub fn modeled_files() -> &'static [&'static str] {
            &[$(<$ty as Record>::FILE),*]
        }

        impl Sde {
            $(
                #[doc = concat!("Read-only records from `", stringify!($field), "`, by ID.")]
                pub fn $field(&self) -> &HashMap<<$ty as Record>::Id, $ty> {
                    &self.$field
                }
            )*

            fn read_tables<R: Read + Seek>(archive: &mut Archive<R>) -> Result<Sde> {
                Ok(Sde {
                    build: archive.build().clone(),
                    unmodeled: unmodeled_files(archive),
                    index: Index::default(),
                    $($field: archive.table::<$ty>()?,)*
                })
            }
        }
    };
}

tables! {
    accounting_entry_types: AccountingEntryType,
    agent_types: AgentType,
    agents_in_space: AgentInSpace,
    ancestries: Ancestry,
    applied_proximity_effects: AppliedProximityEffect,
    archetypes: Archetype,
    bloodlines: Bloodline,
    blueprints: Blueprint,
    categories: Category,
    certificates: Certificate,
    character_attributes: CharacterAttribute,
    character_titles: CharacterTitle,
    clone_grades: CloneGrade,
    compressible_types: CompressibleType,
    contraband_types: ContrabandType,
    control_tower_resources: ControlTowerResources,
    corporation_activities: CorporationActivity,
    corporation_role_groups: CorporationRoleGroup,
    corporation_roles: CorporationRole,
    dbuff_collections: DynamicBuff,
    dogma_attribute_categories: AttributeCategory,
    dogma_attributes: Attribute,
    dogma_effects: Effect,
    dogma_units: DogmaUnit,
    dungeons: Dungeon,
    dynamic_item_attributes: DynamicItemAttributes,
    epic_arcs: EpicArc,
    expert_systems: ExpertSystem,
    factions: Faction,
    fighter_abilities: FighterAbility,
    fighter_abilities_by_type: FighterAbilitiesByType,
    freelance_job_schemas: FreelanceJobSchemas,
    graphic_material_sets: GraphicMaterialSet,
    graphics: Graphic,
    groups: Group,
    icons: Icon,
    industry_activities: IndustryActivity,
    industry_assembly_lines: IndustryAssemblyLine,
    industry_installation_types: IndustryInstallationType,
    industry_modifier_sources: IndustryModifierSource,
    industry_target_filters: IndustryTargetFilter,
    landmarks: Landmark,
    link_with_ship: LinkWithShip,
    asteroid_belts: AsteroidBelt,
    constellations: Constellation,
    moons: Moon,
    planets: Planet,
    regions: Region,
    secondary_suns: SecondarySun,
    solar_systems: SolarSystem,
    stargates: Stargate,
    stars: Star,
    market_groups: MarketGroup,
    masteries: Mastery,
    mercenary_tactical_operations: MercenaryTacticalOperation,
    meta_groups: MetaGroup,
    metenox_moon_drill: MetenoxMoonDrill,
    military_campaign_objectives: MilitaryCampaignObjective,
    military_campaigns: MilitaryCampaign,
    missions: Mission,
    notification_types: NotificationType,
    npc_characters: NpcCharacter,
    npc_corporation_divisions: NpcCorporationDivision,
    npc_corporations: NpcCorporation,
    npc_stations: NpcStation,
    planet_resources: PlanetResource,
    planet_schematics: PlanetSchematic,
    proximity_traps: ProximityTrap,
    races: Race,
    school_map: SchoolMap,
    schools: School,
    ship_tree_elements: ShipTreeElement,
    ship_tree_factions: ShipTreeFaction,
    ship_tree_groups: ShipTreeGroup,
    skill_plans: SkillPlan,
    skin_licenses: SkinLicense,
    skin_materials: SkinMaterial,
    skinr_component_categories: SkinrComponentCategory,
    skinr_component_point_values: SkinrComponentPointValues,
    skinr_component_rarities: SkinrComponentRarity,
    skinr_components: SkinrComponent,
    skinr_slot_categories: SkinrSlotCategory,
    skinr_slot_configurations: SkinrSlotConfiguration,
    skinr_slot_names: SkinrSlotName,
    skinr_slots: SkinrSlot,
    skinr_slots_to_materials: SkinrSlotsToMaterials,
    skinr_tier_thresholds: SkinrTierThreshold,
    skins: Skin,
    sovereignty_upgrades: SovereigntyUpgrade,
    station_operations: StationOperation,
    station_services: StationService,
    station_standings_restrictions: StationStandingsRestriction,
    system_dbuff_emitters: SystemDbuffEmitter,
    system_wide_effects: SystemWideEffect,
    translation_languages: TranslationLanguage,
    type_bonuses: TypeBonuses,
    type_dogma: TypeDogma,
    type_elements: TypeElement,
    type_lists: TypeList,
    type_materials: TypeMaterials,
    types: Type,
}

/// Sorted names of the table files that no model reads.
fn unmodeled_files<R: Read + Seek>(archive: &Archive<R>) -> Vec<String> {
    let mut names: Vec<String> = archive
        .table_files()
        .filter(|name| !modeled_files().contains(name))
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

impl Sde {
    /// Read and index an SDE ZIP from disk. Takes a couple of seconds and a
    /// few hundred MB of memory; load once and share the result.
    pub fn load(path: impl AsRef<Path>) -> Result<Sde> {
        Sde::from_reader(BufReader::new(File::open(path)?))
    }

    /// Read and index an SDE ZIP already in memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<Sde> {
        Sde::from_reader(Cursor::new(bytes))
    }

    /// Read and index an SDE ZIP from any seekable source.
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Sde> {
        let mut archive = Archive::new(reader)?;
        let mut sde = Sde::read_tables(&mut archive)?;
        sde.index = Index::build(&sde);
        Ok(sde)
    }

    /// The build this archive holds.
    pub fn build(&self) -> &BuildInfo {
        &self.build
    }

    /// Files in the archive that this crate does not read. Not empty means CCP
    /// added a table this version does not know.
    pub fn unmodeled_files(&self) -> &[String] {
        &self.unmodeled
    }
}

/// Shows the build only; the tables are far too big to print.
impl fmt::Debug for Sde {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sde")
            .field("build", &self.build)
            .finish_non_exhaustive()
    }
}
