use crate::save_post_load_runtime_world_ownership::build_runtime_world_ownership;
use crate::{
    SavePostLoadRuntimeApplyScript, SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeBuildingSeed,
    SavePostLoadRuntimeCustomChunkSeed, SavePostLoadRuntimeEntityRemapSeed,
    SavePostLoadRuntimeEntitySeed, SavePostLoadRuntimeMarkerSeed, SavePostLoadRuntimeSeedPlan,
    SavePostLoadRuntimeStaticFogSeed, SavePostLoadRuntimeTeamPlanSeed,
    SavePostLoadRuntimeExecutionStepStatus,
    SavePostLoadRuntimeWorldOwnership, SavePostLoadRuntimeWorldSeed,
    SavePostLoadRuntimeWorldSurfaceKind, SavePostLoadWorldObservation,
};
use std::collections::{btree_map::Entry, BTreeMap};

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadRuntimeWorldShell {
    pub seed: SavePostLoadRuntimeWorldSeed,
    pub ownership_claim_steps: BTreeMap<SavePostLoadRuntimeWorldSurfaceKind, usize>,
    pub team_plans: Vec<SavePostLoadRuntimeTeamPlanSeed>,
    pub team_plans_by_team: BTreeMap<u32, Vec<SavePostLoadRuntimeTeamPlanSeed>>,
    pub markers: Vec<SavePostLoadRuntimeMarkerSeed>,
    pub markers_by_id: BTreeMap<i32, SavePostLoadRuntimeMarkerSeed>,
    pub static_fog: Option<SavePostLoadRuntimeStaticFogSeed>,
    pub buildings: Vec<SavePostLoadRuntimeBuildingSeed>,
    pub buildings_by_center_index: BTreeMap<usize, SavePostLoadRuntimeBuildingSeed>,
    pub loadable_entities: Vec<SavePostLoadRuntimeEntitySeed>,
    pub loadable_entities_by_id: BTreeMap<i32, SavePostLoadRuntimeEntitySeed>,
    pub loadable_entities_by_effective_class_id: BTreeMap<u8, Vec<SavePostLoadRuntimeEntitySeed>>,
    pub loadable_entities_by_effective_name: BTreeMap<String, Vec<SavePostLoadRuntimeEntitySeed>>,
}

impl SavePostLoadRuntimeWorldShell {
    pub fn applied_step_count(&self) -> usize {
        self.team_plans.len()
            + self.markers.len()
            + usize::from(self.static_fog.is_some())
            + self.buildings.len()
            + self.loadable_entities.len()
    }

    pub fn owned_step_count(&self, kind: SavePostLoadRuntimeWorldSurfaceKind) -> usize {
        self.ownership_claim_steps
            .get(&kind)
            .copied()
            .unwrap_or_default()
    }

    fn claim_world_surface_step(&mut self, kind: SavePostLoadRuntimeWorldSurfaceKind) {
        *self.ownership_claim_steps.entry(kind).or_default() += 1;
    }

    pub fn loadable_entities_for_effective_class_id(
        &self,
        class_id: u8,
    ) -> Option<&[SavePostLoadRuntimeEntitySeed]> {
        self.loadable_entities_by_effective_class_id
            .get(&class_id)
            .map(Vec::as_slice)
    }

    pub fn loadable_entities_for_effective_name(
        &self,
        name: &str,
    ) -> Option<&[SavePostLoadRuntimeEntitySeed]> {
        self.loadable_entities_by_effective_name
            .get(name)
            .map(Vec::as_slice)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePostLoadRuntimeApplyIssue {
    MissingSeed(SavePostLoadRuntimeApplyStep),
    MissingWorldShell(SavePostLoadRuntimeApplyStep),
    DuplicateEntityRemapCustomId(u16),
    DuplicateCustomChunkName(String),
    DuplicateMarkerId(i32),
    DuplicateBuildingCenterIndex(usize),
    DuplicateEntityId(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeExecutionSourceRegion {
    pub source_region_name: &'static str,
    pub executed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub failed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub awaiting_world_shell_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub blocked_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub deferred_steps: Vec<SavePostLoadRuntimeApplyStep>,
}

impl SavePostLoadRuntimeExecutionSourceRegion {
    pub fn step_count(&self, status: SavePostLoadRuntimeExecutionStepStatus) -> usize {
        self.steps_with_status(status).len()
    }

    pub fn total_step_count(&self) -> usize {
        self.executed_steps.len()
            + self.failed_steps.len()
            + self.awaiting_world_shell_steps.len()
            + self.blocked_steps.len()
            + self.deferred_steps.len()
    }

    pub fn summary_label(&self) -> String {
        format!(
            "region={} exec={} fail={} wait={} block={} defer={} total={}",
            self.source_region_name,
            self.executed_steps.len(),
            self.failed_steps.len(),
            self.awaiting_world_shell_steps.len(),
            self.blocked_steps.len(),
            self.deferred_steps.len(),
            self.total_step_count(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "region={} exec={} fail={} wait={} block={} defer={} total={}",
            self.source_region_name,
            self.executed_steps.len(),
            self.failed_steps.len(),
            self.awaiting_world_shell_steps.len(),
            self.blocked_steps.len(),
            self.deferred_steps.len(),
            self.total_step_count(),
        )
    }

    pub fn steps_with_status(
        &self,
        status: SavePostLoadRuntimeExecutionStepStatus,
    ) -> &[SavePostLoadRuntimeApplyStep] {
        match status {
            SavePostLoadRuntimeExecutionStepStatus::Executed => &self.executed_steps,
            SavePostLoadRuntimeExecutionStepStatus::Failed => &self.failed_steps,
            SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell => {
                &self.awaiting_world_shell_steps
            }
            SavePostLoadRuntimeExecutionStepStatus::Blocked => &self.blocked_steps,
            SavePostLoadRuntimeExecutionStepStatus::Deferred => &self.deferred_steps,
        }
    }

    fn steps_with_status_mut(
        &mut self,
        status: SavePostLoadRuntimeExecutionStepStatus,
    ) -> &mut Vec<SavePostLoadRuntimeApplyStep> {
        match status {
            SavePostLoadRuntimeExecutionStepStatus::Executed => &mut self.executed_steps,
            SavePostLoadRuntimeExecutionStepStatus::Failed => &mut self.failed_steps,
            SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell => {
                &mut self.awaiting_world_shell_steps
            }
            SavePostLoadRuntimeExecutionStepStatus::Blocked => &mut self.blocked_steps,
            SavePostLoadRuntimeExecutionStepStatus::Deferred => &mut self.deferred_steps,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadRuntimeApplyExecution {
    pub can_seed_runtime_apply: bool,
    pub world_shell_ready: bool,
    pub executed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub failed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub awaiting_world_shell_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub blocked_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub deferred_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub world_shell: Option<SavePostLoadRuntimeWorldShell>,
    pub entity_remaps: Vec<SavePostLoadRuntimeEntityRemapSeed>,
    pub entity_remaps_by_custom_id: BTreeMap<u16, SavePostLoadRuntimeEntityRemapSeed>,
    pub custom_chunks: Vec<SavePostLoadRuntimeCustomChunkSeed>,
    pub custom_chunks_by_name: BTreeMap<String, SavePostLoadRuntimeCustomChunkSeed>,
    pub skipped_entities: Vec<SavePostLoadRuntimeEntitySeed>,
    pub issues: Vec<SavePostLoadRuntimeApplyIssue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadRuntimeWorldSemanticsExecution {
    pub world_shell_ready: bool,
    pub executed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub failed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub awaiting_world_shell_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub blocked_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub deferred_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub world_shell: Option<SavePostLoadRuntimeWorldShell>,
    pub ownership: SavePostLoadRuntimeWorldOwnership,
    pub issues: Vec<SavePostLoadRuntimeApplyIssue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadLiveRuntimeActivation {
    pub world_shell: SavePostLoadRuntimeWorldShell,
    pub ownership: SavePostLoadRuntimeWorldOwnership,
    pub entity_remaps: Vec<SavePostLoadRuntimeEntityRemapSeed>,
    pub entity_remaps_by_custom_id: BTreeMap<u16, SavePostLoadRuntimeEntityRemapSeed>,
    pub custom_chunks: Vec<SavePostLoadRuntimeCustomChunkSeed>,
    pub custom_chunks_by_name: BTreeMap<String, SavePostLoadRuntimeCustomChunkSeed>,
    pub skipped_entities: Vec<SavePostLoadRuntimeEntitySeed>,
}

impl SavePostLoadLiveRuntimeActivation {
    fn from_executions(
        runtime_apply: SavePostLoadRuntimeApplyExecution,
        runtime_world_semantics: SavePostLoadRuntimeWorldSemanticsExecution,
    ) -> Option<Self> {
        if !runtime_apply.can_activate_live_runtime()
            || !runtime_world_semantics.can_activate_live_runtime()
        {
            return None;
        }

        let world_shell = runtime_apply.world_shell?;
        if runtime_world_semantics.world_shell.as_ref() != Some(&world_shell) {
            return None;
        }

        Some(Self {
            world_shell,
            ownership: runtime_world_semantics.ownership,
            entity_remaps: runtime_apply.entity_remaps,
            entity_remaps_by_custom_id: runtime_apply.entity_remaps_by_custom_id,
            custom_chunks: runtime_apply.custom_chunks,
            custom_chunks_by_name: runtime_apply.custom_chunks_by_name,
            skipped_entities: runtime_apply.skipped_entities,
        })
    }
}

impl SavePostLoadRuntimeWorldSemanticsExecution {
    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadRuntimeExecutionSourceRegion> {
        self.source_regions()
            .into_iter()
            .find(|region| region.source_region_name == source_region_name)
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeExecutionSourceRegion> {
        build_source_regions(
            &self.executed_steps,
            &self.failed_steps,
            &self.awaiting_world_shell_steps,
            &self.blocked_steps,
            &self.deferred_steps,
        )
    }

    pub fn can_apply_world_semantics(&self) -> bool {
        self.ownership.can_apply_world_semantics()
    }

    pub fn can_activate_live_runtime(&self) -> bool {
        self.world_shell_ready
            && self.can_apply_world_semantics()
            && self.has_world_shell()
            && self.awaiting_world_shell_steps.is_empty()
            && self.blocked_steps.is_empty()
            && self.failed_steps.is_empty()
    }

    pub fn targeted_step_count(&self) -> usize {
        self.executed_steps.len()
            + self.failed_steps.len()
            + self.awaiting_world_shell_steps.len()
            + self.blocked_steps.len()
            + self.deferred_steps.len()
    }

    pub fn executed_step_count(&self) -> usize {
        self.executed_steps.len()
    }

    pub fn failed_step_count(&self) -> usize {
        self.failed_steps.len()
    }

    pub fn pending_step_count(&self) -> usize {
        self.awaiting_world_shell_steps.len() + self.blocked_steps.len() + self.deferred_steps.len()
    }

    pub fn deferred_step_count(&self) -> usize {
        self.deferred_steps.len()
    }

    pub fn has_world_shell(&self) -> bool {
        self.world_shell.is_some()
    }

    pub fn summary_label(&self) -> String {
        format!(
            "shell={} semantics={} exec={} fail={} wait={} block={} defer={} total={} sources={} issues={} live={}",
            bool_label(self.world_shell_ready),
            bool_label(self.can_apply_world_semantics()),
            self.executed_step_count(),
            self.failed_step_count(),
            self.awaiting_world_shell_steps.len(),
            self.blocked_steps.len(),
            self.deferred_step_count(),
            self.targeted_step_count(),
            self.source_regions().len(),
            self.issues.len(),
            bool_label(self.can_activate_live_runtime()),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "shell={} semantics={} exec={} fail={} wait={} block={} defer={} total={} sources=[{}] issues={} live={}",
            bool_label(self.world_shell_ready),
            bool_label(self.can_apply_world_semantics()),
            self.executed_step_count(),
            self.failed_step_count(),
            self.awaiting_world_shell_steps.len(),
            self.blocked_steps.len(),
            self.deferred_step_count(),
            self.targeted_step_count(),
            self.source_regions()
                .iter()
                .map(SavePostLoadRuntimeExecutionSourceRegion::summary_label)
                .collect::<Vec<_>>()
                .join(","),
            self.issues.len(),
            bool_label(self.can_activate_live_runtime()),
        )
    }
}

impl SavePostLoadRuntimeApplyExecution {
    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadRuntimeExecutionSourceRegion> {
        self.source_regions()
            .into_iter()
            .find(|region| region.source_region_name == source_region_name)
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeExecutionSourceRegion> {
        build_source_regions(
            &self.executed_steps,
            &self.failed_steps,
            &self.awaiting_world_shell_steps,
            &self.blocked_steps,
            &self.deferred_steps,
        )
    }

    pub fn executed_step_count(&self) -> usize {
        self.executed_steps.len()
    }

    pub fn failed_step_count(&self) -> usize {
        self.failed_steps.len()
    }

    pub fn pending_step_count(&self) -> usize {
        self.awaiting_world_shell_steps.len() + self.blocked_steps.len() + self.deferred_steps.len()
    }

    pub fn deferred_step_count(&self) -> usize {
        self.deferred_steps.len()
    }

    pub fn has_world_shell(&self) -> bool {
        self.world_shell.is_some()
    }

    pub fn can_activate_live_runtime(&self) -> bool {
        self.can_seed_runtime_apply
            && self.world_shell_ready
            && self.has_world_shell()
            && self.awaiting_world_shell_steps.is_empty()
            && self.blocked_steps.is_empty()
            && self.failed_steps.is_empty()
    }

    pub fn targeted_step_count(&self) -> usize {
        self.executed_steps.len()
            + self.failed_steps.len()
            + self.awaiting_world_shell_steps.len()
            + self.blocked_steps.len()
            + self.deferred_steps.len()
    }

    pub fn summary_label(&self) -> String {
        format!(
            "seed={} shell={} exec={} fail={} wait={} block={} defer={} total={} sources={} issues={} live={}",
            bool_label(self.can_seed_runtime_apply),
            bool_label(self.world_shell_ready),
            self.executed_step_count(),
            self.failed_step_count(),
            self.awaiting_world_shell_steps.len(),
            self.blocked_steps.len(),
            self.deferred_step_count(),
            self.targeted_step_count(),
            self.source_regions().len(),
            self.issues.len(),
            bool_label(self.can_activate_live_runtime()),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "seed={} shell={} exec={} fail={} wait={} block={} defer={} total={} sources=[{}] issues={} live={}",
            bool_label(self.can_seed_runtime_apply),
            bool_label(self.world_shell_ready),
            self.executed_step_count(),
            self.failed_step_count(),
            self.awaiting_world_shell_steps.len(),
            self.blocked_steps.len(),
            self.deferred_step_count(),
            self.targeted_step_count(),
            self.source_regions()
                .iter()
                .map(SavePostLoadRuntimeExecutionSourceRegion::summary_label)
                .collect::<Vec<_>>()
                .join(","),
            self.issues.len(),
            bool_label(self.can_activate_live_runtime()),
        )
    }

    fn from_script(script: SavePostLoadRuntimeApplyScript) -> Self {
        Self {
            can_seed_runtime_apply: script.can_seed_runtime_apply,
            world_shell_ready: script.world_shell_ready,
            executed_steps: Vec::new(),
            failed_steps: Vec::new(),
            awaiting_world_shell_steps: script.awaiting_world_shell_steps,
            blocked_steps: script.blocked_steps,
            deferred_steps: script.deferred_steps,
            world_shell: None,
            entity_remaps: Vec::new(),
            entity_remaps_by_custom_id: BTreeMap::new(),
            custom_chunks: Vec::new(),
            custom_chunks_by_name: BTreeMap::new(),
            skipped_entities: Vec::new(),
            issues: Vec::new(),
        }
    }

    fn apply_step(
        &mut self,
        plan: &SavePostLoadRuntimeSeedPlan,
        step: &SavePostLoadRuntimeApplyStep,
    ) -> bool {
        match step {
            SavePostLoadRuntimeApplyStep::WorldShell => {
                self.world_shell = Some(SavePostLoadRuntimeWorldShell {
                    seed: plan.world_seed.clone(),
                    ownership_claim_steps: BTreeMap::new(),
                    team_plans: Vec::new(),
                    team_plans_by_team: BTreeMap::new(),
                    markers: Vec::new(),
                    markers_by_id: BTreeMap::new(),
                    static_fog: None,
                    buildings: Vec::new(),
                    buildings_by_center_index: BTreeMap::new(),
                    loadable_entities: Vec::new(),
                    loadable_entities_by_id: BTreeMap::new(),
                    loadable_entities_by_effective_class_id: BTreeMap::new(),
                    loadable_entities_by_effective_name: BTreeMap::new(),
                });
                self.world_shell
                    .as_mut()
                    .expect("world shell just inserted")
                    .claim_world_surface_step(SavePostLoadRuntimeWorldSurfaceKind::WorldShell);
                true
            }
            SavePostLoadRuntimeApplyStep::EntityRemap { remap_index } => {
                let Some(seed) = plan.entity_remap_seeds.get(*remap_index) else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                match self.entity_remaps_by_custom_id.entry(seed.custom_id) {
                    Entry::Vacant(entry) => {
                        entry.insert(seed.clone());
                        self.entity_remaps.push(seed.clone());
                        true
                    }
                    Entry::Occupied(_) => {
                        self.issues.push(
                            SavePostLoadRuntimeApplyIssue::DuplicateEntityRemapCustomId(
                                seed.custom_id,
                            ),
                        );
                        false
                    }
                }
            }
            SavePostLoadRuntimeApplyStep::TeamPlan {
                group_index,
                plan_index,
            } => {
                let Some(seed) = plan.team_plan_seeds.iter().find(|seed| {
                    seed.group_index == *group_index && seed.plan_index == *plan_index
                }) else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                let Some(shell) = self.world_shell.as_mut() else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingWorldShell(
                            step.clone(),
                        ));
                    return false;
                };
                shell.team_plans.push(seed.clone());
                shell
                    .team_plans_by_team
                    .entry(seed.team_id)
                    .or_default()
                    .push(seed.clone());
                shell.claim_world_surface_step(SavePostLoadRuntimeWorldSurfaceKind::TeamPlans);
                true
            }
            SavePostLoadRuntimeApplyStep::Marker { marker_index } => {
                let Some(seed) = plan
                    .marker_seeds
                    .iter()
                    .find(|seed| seed.marker_index == *marker_index)
                else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                let Some(shell) = self.world_shell.as_mut() else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingWorldShell(
                            step.clone(),
                        ));
                    return false;
                };
                match shell.markers_by_id.entry(seed.id) {
                    Entry::Vacant(entry) => {
                        entry.insert(seed.clone());
                        shell.markers.push(seed.clone());
                        shell
                            .claim_world_surface_step(SavePostLoadRuntimeWorldSurfaceKind::Markers);
                        true
                    }
                    Entry::Occupied(_) => {
                        self.issues
                            .push(SavePostLoadRuntimeApplyIssue::DuplicateMarkerId(seed.id));
                        false
                    }
                }
            }
            SavePostLoadRuntimeApplyStep::StaticFog => {
                let Some(seed) = plan.static_fog_seed.as_ref() else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                let Some(shell) = self.world_shell.as_mut() else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingWorldShell(
                            step.clone(),
                        ));
                    return false;
                };
                shell.static_fog = Some(seed.clone());
                shell.claim_world_surface_step(SavePostLoadRuntimeWorldSurfaceKind::StaticFog);
                true
            }
            SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index } => {
                let Some(seed) = plan
                    .custom_chunk_seeds
                    .iter()
                    .find(|seed| seed.chunk_index == *chunk_index)
                else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                match self.custom_chunks_by_name.entry(seed.name.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(seed.clone());
                        self.custom_chunks.push(seed.clone());
                        true
                    }
                    Entry::Occupied(_) => {
                        self.issues
                            .push(SavePostLoadRuntimeApplyIssue::DuplicateCustomChunkName(
                                seed.name.clone(),
                            ));
                        false
                    }
                }
            }
            SavePostLoadRuntimeApplyStep::Building { center_index } => {
                let Some(seed) = plan
                    .building_seeds
                    .iter()
                    .find(|seed| seed.activation.center_index == *center_index)
                else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                let Some(shell) = self.world_shell.as_mut() else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingWorldShell(
                            step.clone(),
                        ));
                    return false;
                };
                match shell
                    .buildings_by_center_index
                    .entry(seed.activation.center_index)
                {
                    Entry::Vacant(entry) => {
                        entry.insert(seed.clone());
                        shell.buildings.push(seed.clone());
                        shell.claim_world_surface_step(
                            SavePostLoadRuntimeWorldSurfaceKind::Buildings,
                        );
                        true
                    }
                    Entry::Occupied(_) => {
                        self.issues.push(
                            SavePostLoadRuntimeApplyIssue::DuplicateBuildingCenterIndex(
                                seed.activation.center_index,
                            ),
                        );
                        false
                    }
                }
            }
            SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index } => {
                let Some(seed) = plan
                    .loadable_entity_seeds
                    .iter()
                    .find(|seed| seed.entity_index == *entity_index)
                else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                let Some(shell) = self.world_shell.as_mut() else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingWorldShell(
                            step.clone(),
                        ));
                    return false;
                };
                match shell
                    .loadable_entities_by_id
                    .entry(seed.activation.entity_id)
                {
                    Entry::Vacant(entry) => {
                        entry.insert(seed.clone());
                        shell.loadable_entities.push(seed.clone());
                        if let Some(effective_class_id) = seed.activation.effective_class_id {
                            shell
                                .loadable_entities_by_effective_class_id
                                .entry(effective_class_id)
                                .or_default()
                                .push(seed.clone());
                        }
                        if let Some(effective_name) = seed.activation.effective_name.clone() {
                            shell
                                .loadable_entities_by_effective_name
                                .entry(effective_name)
                                .or_default()
                                .push(seed.clone());
                        }
                        shell.claim_world_surface_step(
                            SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
                        );
                        true
                    }
                    Entry::Occupied(_) => {
                        self.issues
                            .push(SavePostLoadRuntimeApplyIssue::DuplicateEntityId(
                                seed.activation.entity_id,
                            ));
                        false
                    }
                }
            }
            SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index } => {
                let Some(seed) = plan
                    .skipped_entity_seeds
                    .iter()
                    .find(|seed| seed.entity_index == *entity_index)
                else {
                    self.issues
                        .push(SavePostLoadRuntimeApplyIssue::MissingSeed(step.clone()));
                    return false;
                };
                self.skipped_entities.push(seed.clone());
                true
            }
        }
    }
}

impl SavePostLoadWorldObservation {
    pub fn live_runtime_activation(&self) -> Option<SavePostLoadLiveRuntimeActivation> {
        self.runtime_seed_plan().live_runtime_activation()
    }

    pub fn execute_runtime_apply(&self) -> SavePostLoadRuntimeApplyExecution {
        self.runtime_seed_plan().execute_runtime_apply()
    }

    pub fn execute_runtime_world_semantics(&self) -> SavePostLoadRuntimeWorldSemanticsExecution {
        self.runtime_seed_plan().execute_runtime_world_semantics()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn live_runtime_activation(&self) -> Option<SavePostLoadLiveRuntimeActivation> {
        SavePostLoadLiveRuntimeActivation::from_executions(
            self.execute_runtime_apply(),
            self.execute_runtime_world_semantics(),
        )
    }

    pub fn execute_runtime_apply(&self) -> SavePostLoadRuntimeApplyExecution {
        let script = self.runtime_apply_script();
        let mut execution = SavePostLoadRuntimeApplyExecution::from_script(script.clone());
        for step in &script.apply_now_steps {
            if execution.apply_step(self, step) {
                execution.executed_steps.push(step.clone());
            } else {
                execution.failed_steps.push(step.clone());
            }
        }
        execution
    }

    pub fn execute_runtime_world_semantics(&self) -> SavePostLoadRuntimeWorldSemanticsExecution {
        let script = self.runtime_apply_script();
        let apply_now_steps = filter_world_semantics_steps(&script.apply_now_steps);
        let mut execution = SavePostLoadRuntimeApplyExecution {
            can_seed_runtime_apply: script.world_shell_ready,
            world_shell_ready: script.world_shell_ready,
            executed_steps: Vec::new(),
            failed_steps: Vec::new(),
            awaiting_world_shell_steps: filter_world_semantics_steps(
                &script.awaiting_world_shell_steps,
            ),
            blocked_steps: filter_world_semantics_steps(&script.blocked_steps),
            deferred_steps: Vec::new(),
            world_shell: None,
            entity_remaps: Vec::new(),
            entity_remaps_by_custom_id: BTreeMap::new(),
            custom_chunks: Vec::new(),
            custom_chunks_by_name: BTreeMap::new(),
            skipped_entities: Vec::new(),
            issues: Vec::new(),
        };

        for step in &apply_now_steps {
            if execution.apply_step(self, step) {
                execution.executed_steps.push(step.clone());
            } else {
                execution.failed_steps.push(step.clone());
            }
        }

        let mut world_execution = SavePostLoadRuntimeWorldSemanticsExecution {
            world_shell_ready: execution.world_shell_ready,
            executed_steps: execution.executed_steps,
            failed_steps: execution.failed_steps,
            awaiting_world_shell_steps: execution.awaiting_world_shell_steps,
            blocked_steps: execution.blocked_steps,
            deferred_steps: filter_world_semantics_steps(&script.deferred_steps),
            world_shell: execution.world_shell,
            issues: execution.issues,
            ownership: SavePostLoadRuntimeWorldOwnership {
                world_shell_ready: execution.world_shell_ready,
                surfaces: Vec::new(),
            },
        };
        world_execution.ownership = build_runtime_world_ownership(self, &world_execution);
        world_execution
    }
}

fn filter_world_semantics_steps(
    steps: &[SavePostLoadRuntimeApplyStep],
) -> Vec<SavePostLoadRuntimeApplyStep> {
    steps
        .iter()
        .filter(|step| step.targets_world_semantics())
        .cloned()
        .collect()
}

fn build_source_regions(
    executed_steps: &[SavePostLoadRuntimeApplyStep],
    failed_steps: &[SavePostLoadRuntimeApplyStep],
    awaiting_world_shell_steps: &[SavePostLoadRuntimeApplyStep],
    blocked_steps: &[SavePostLoadRuntimeApplyStep],
    deferred_steps: &[SavePostLoadRuntimeApplyStep],
) -> Vec<SavePostLoadRuntimeExecutionSourceRegion> {
    let mut source_regions = Vec::new();

    for (status, steps) in [
        (
            SavePostLoadRuntimeExecutionStepStatus::Executed,
            executed_steps,
        ),
        (SavePostLoadRuntimeExecutionStepStatus::Failed, failed_steps),
        (
            SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell,
            awaiting_world_shell_steps,
        ),
        (SavePostLoadRuntimeExecutionStepStatus::Blocked, blocked_steps),
        (SavePostLoadRuntimeExecutionStepStatus::Deferred, deferred_steps),
    ] {
        for step in steps {
            let source_region_name = source_region_name_for_step(step);
            let source_region = match source_regions.iter_mut().find(
                |candidate: &&mut SavePostLoadRuntimeExecutionSourceRegion| {
                    candidate.source_region_name == source_region_name
                },
            ) {
                Some(source_region) => source_region,
                None => {
                    source_regions.push(SavePostLoadRuntimeExecutionSourceRegion {
                        source_region_name,
                        executed_steps: Vec::new(),
                        failed_steps: Vec::new(),
                        awaiting_world_shell_steps: Vec::new(),
                        blocked_steps: Vec::new(),
                        deferred_steps: Vec::new(),
                    });
                    source_regions
                        .last_mut()
                        .expect("source region was just pushed")
                }
            };
            source_region.steps_with_status_mut(status).push(step.clone());
        }
    }

    source_regions.sort_by_key(|region| source_region_sort_key(region.source_region_name));
    source_regions
}

fn source_region_name_for_step(step: &SavePostLoadRuntimeApplyStep) -> &'static str {
    match step {
        SavePostLoadRuntimeApplyStep::WorldShell | SavePostLoadRuntimeApplyStep::Building { .. } => {
            "map"
        }
        SavePostLoadRuntimeApplyStep::EntityRemap { .. }
        | SavePostLoadRuntimeApplyStep::TeamPlan { .. }
        | SavePostLoadRuntimeApplyStep::LoadableEntity { .. }
        | SavePostLoadRuntimeApplyStep::SkippedEntity { .. } => "entities",
        SavePostLoadRuntimeApplyStep::Marker { .. } => "markers",
        SavePostLoadRuntimeApplyStep::StaticFog | SavePostLoadRuntimeApplyStep::CustomChunk { .. } => {
            "custom"
        }
    }
}

fn source_region_sort_key(source_region_name: &str) -> u8 {
    match source_region_name {
        "map" => 0,
        "entities" => 1,
        "markers" => 2,
        "custom" => 3,
        _ => 4,
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, ContentHeaderEntry,
        CoreTailSnapshot, CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail,
        ParsedCustomChunk, PointMarkerModel, SaveEntityChunkObservation, SaveEntityClassKind,
        SaveEntityClassSummary, SaveEntityPostLoadClassSummary, SaveEntityPostLoadKind,
        SaveEntityPostLoadSummary, SaveEntityRemapEntry, SaveEntityRemapSummary,
        SaveMapRegionObservation, SavePostLoadRuntimeWorldOwnershipStatus,
        SavePostLoadRuntimeWorldSurfaceKind, StaticFogChunk, StaticFogTeam, TeamPlan,
        TeamPlanGroup, TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn execute_runtime_apply_materializes_clean_seedable_state() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let execution = observation.execute_runtime_apply();
        let shell = execution.world_shell.as_ref().unwrap();

        assert!(execution.can_seed_runtime_apply);
        assert!(execution.world_shell_ready);
        assert!(execution.can_activate_live_runtime());
        assert!(execution.awaiting_world_shell_steps.is_empty());
        assert!(execution.blocked_steps.is_empty());
        assert!(execution.deferred_steps.is_empty());
        assert!(execution.failed_steps.is_empty());
        assert!(execution.issues.is_empty());
        assert_eq!(execution.executed_step_count(), 14);
        assert_eq!(execution.failed_step_count(), 0);
        assert_eq!(execution.pending_step_count(), 0);
        assert_eq!(
            execution.summary_label(),
            "seed=yes shell=yes exec=14 fail=0 wait=0 block=0 defer=0 total=14 sources=4 issues=0 live=yes"
        );
        assert!(execution
            .detail_label()
            .contains("region=custom exec=3 fail=0 wait=0 block=0 defer=0 total=3"));
        assert_eq!(execution.entity_remaps.len(), 2);
        assert_eq!(execution.entity_remaps_by_custom_id.len(), 2);
        assert_eq!(execution.custom_chunks.len(), 2);
        assert_eq!(execution.custom_chunks_by_name.len(), 2);
        assert_eq!(shell.team_plans.len(), 2);
        assert_eq!(shell.team_plans_by_team.len(), 2);
        assert_eq!(shell.markers.len(), 2);
        assert_eq!(shell.markers_by_id.len(), 2);
        assert!(shell.static_fog.is_some());
        assert_eq!(shell.buildings.len(), 1);
        assert_eq!(shell.buildings_by_center_index.len(), 1);
        assert_eq!(shell.loadable_entities.len(), 3);
        assert_eq!(shell.loadable_entities_by_id.len(), 3);
        assert_eq!(shell.loadable_entities_by_effective_class_id.len(), 2);
        assert_eq!(shell.loadable_entities_by_effective_name.len(), 2);
        assert_eq!(
            shell
                .loadable_entities_for_effective_class_id(3)
                .unwrap()
                .iter()
                .map(|seed| seed.activation.entity_id)
                .collect::<Vec<_>>(),
            vec![42, 43]
        );
        assert_eq!(
            shell
                .loadable_entities_for_effective_class_id(4)
                .unwrap()
                .iter()
                .map(|seed| seed.activation.entity_id)
                .collect::<Vec<_>>(),
            vec![44]
        );
        assert_eq!(
            shell
                .loadable_entities_for_effective_name("flare")
                .unwrap()
                .iter()
                .map(|seed| seed.activation.entity_id)
                .collect::<Vec<_>>(),
            vec![42, 43]
        );
        assert_eq!(
            shell
                .loadable_entities_for_effective_name("mace")
                .unwrap()
                .iter()
                .map(|seed| seed.activation.entity_id)
                .collect::<Vec<_>>(),
            vec![44]
        );
        assert_eq!(shell.applied_step_count(), 9);
        assert_eq!(shell.seed.tile_count(), 4);
    }

    #[test]
    fn save_post_load_runtime_execution_summary_and_detail_labels_use_bool_labels() {
        let execution = SavePostLoadRuntimeApplyExecution {
            can_seed_runtime_apply: true,
            world_shell_ready: false,
            executed_steps: Vec::new(),
            failed_steps: Vec::new(),
            awaiting_world_shell_steps: Vec::new(),
            blocked_steps: Vec::new(),
            deferred_steps: Vec::new(),
            world_shell: None,
            entity_remaps: Vec::new(),
            entity_remaps_by_custom_id: BTreeMap::new(),
            custom_chunks: Vec::new(),
            custom_chunks_by_name: BTreeMap::new(),
            skipped_entities: Vec::new(),
            issues: Vec::new(),
        };
        let source_region = SavePostLoadRuntimeExecutionSourceRegion {
            source_region_name: "map",
            executed_steps: vec![SavePostLoadRuntimeApplyStep::WorldShell],
            failed_steps: Vec::new(),
            awaiting_world_shell_steps: Vec::new(),
            blocked_steps: vec![SavePostLoadRuntimeApplyStep::Building { center_index: 0 }],
            deferred_steps: Vec::new(),
        };

        assert_eq!(
            execution.summary_label(),
            "seed=yes shell=no exec=0 fail=0 wait=0 block=0 defer=0 total=0 sources=0 issues=0 live=no"
        );
        assert_eq!(
            execution.detail_label(),
            "seed=yes shell=no exec=0 fail=0 wait=0 block=0 defer=0 total=0 sources=[] issues=0 live=no"
        );
        assert_eq!(
            source_region.summary_label(),
            "region=map exec=1 fail=0 wait=0 block=1 defer=0 total=2"
        );
        assert_eq!(
            source_region.detail_label(),
            "region=map exec=1 fail=0 wait=0 block=1 defer=0 total=2"
        );
    }

    #[test]
    fn live_runtime_activation_materializes_clean_seedable_runtime_bundle() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let activation = observation
            .live_runtime_activation()
            .expect("expected live runtime activation");

        assert!(activation.ownership.can_activate_live_runtime());
        assert_eq!(activation.world_shell.team_plans.len(), 2);
        assert_eq!(activation.world_shell.markers.len(), 2);
        assert_eq!(activation.world_shell.buildings.len(), 1);
        assert_eq!(activation.world_shell.loadable_entities.len(), 3);
        assert_eq!(activation.entity_remaps.len(), 2);
        assert_eq!(activation.entity_remaps_by_custom_id.len(), 2);
        assert_eq!(activation.custom_chunks.len(), 2);
        assert_eq!(activation.custom_chunks_by_name.len(), 2);
        assert!(activation.custom_chunks_by_name.contains_key("static-fog-data"));
        assert!(activation.skipped_entities.is_empty());
    }

    #[test]
    fn live_runtime_activation_rejects_auxiliary_or_world_failures() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.custom_chunks[1].name = "static-fog-data".to_string();

        assert!(observation.live_runtime_activation().is_none());

        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.map.world.tiles[0].building_center_index = None;

        assert!(observation.live_runtime_activation().is_none());
    }

    #[test]
    fn execute_runtime_apply_preserves_non_applyable_steps_as_pending() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let execution = observation.execute_runtime_apply();

        assert!(!execution.can_seed_runtime_apply);
        assert!(!execution.world_shell_ready);
        assert!(!execution.has_world_shell());
        assert!(execution.failed_steps.is_empty());
        assert!(execution.issues.is_empty());
        assert_eq!(
            execution.executed_steps,
            vec![
                SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 },
                SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 1 },
                SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 },
                SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 1 },
            ]
        );
        assert_eq!(execution.entity_remaps.len(), 2);
        assert_eq!(execution.entity_remaps_by_custom_id.len(), 2);
        assert_eq!(execution.custom_chunks.len(), 2);
        assert_eq!(execution.custom_chunks_by_name.len(), 2);
        assert_eq!(
            execution.awaiting_world_shell_steps,
            vec![
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 0,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 1,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 0 },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 },
                SavePostLoadRuntimeApplyStep::StaticFog,
            ]
        );
        assert_eq!(
            execution.blocked_steps,
            vec![
                SavePostLoadRuntimeApplyStep::WorldShell,
                SavePostLoadRuntimeApplyStep::Building { center_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 },
            ]
        );
        assert_eq!(
            execution.deferred_steps,
            vec![SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 }]
        );
        assert_eq!(execution.deferred_step_count(), 1);
        assert_eq!(execution.pending_step_count(), 10);
    }

    #[test]
    fn execute_runtime_apply_groups_steps_by_source_region() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.world_entity_chunks[1].class_id = 254;
        observation.world_entity_chunks[1].custom_name = Some("mod-unit".to_string());
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;
        observation.custom_chunks.push(CustomChunkEntry {
            name: observation.custom_chunks[1].name.clone(),
            chunk_len: 1,
            chunk_bytes: vec![0xee],
            chunk_sha256: "mystery-duplicate".to_string(),
            parsed: ParsedCustomChunk::Unknown,
        });

        let execution = observation.execute_runtime_apply();
        let source_regions = execution.source_regions();
        let map = execution.source_region("map").unwrap();
        let entities = execution.source_region("entities").unwrap();
        let markers = execution.source_region("markers").unwrap();
        let custom = execution.source_region("custom").unwrap();

        assert_eq!(source_regions.len(), 4);
        let source_region_names = source_regions
            .iter()
            .map(|region| region.source_region_name)
            .collect::<Vec<_>>();
        assert_eq!(source_region_names, vec!["map", "entities", "markers", "custom"]);
        assert_eq!(
            execution.summary_label(),
            format!(
                "seed={} shell={} exec={} fail={} wait={} block={} defer={} total={} sources={} issues={} live={}",
                bool_label(execution.can_seed_runtime_apply),
                bool_label(execution.world_shell_ready),
                execution.executed_step_count(),
                execution.failed_step_count(),
                execution.awaiting_world_shell_steps.len(),
                execution.blocked_steps.len(),
                execution.deferred_step_count(),
                execution.targeted_step_count(),
                execution.source_regions().len(),
                execution.issues.len(),
                bool_label(execution.can_activate_live_runtime()),
            )
        );
        assert_eq!(
            entities.summary_label(),
            "region=entities exec=2 fail=0 wait=2 block=2 defer=1 total=7"
        );
        assert_eq!(
            custom.detail_label(),
            "region=custom exec=2 fail=1 wait=1 block=0 defer=0 total=4"
        );
        assert_eq!(map.step_count(SavePostLoadRuntimeExecutionStepStatus::Blocked), 2);
        assert_eq!(map.total_step_count(), 2);
        assert_eq!(entities.total_step_count(), 7);
        assert_eq!(
            entities.step_count(SavePostLoadRuntimeExecutionStepStatus::Executed),
            2
        );
        assert_eq!(
            entities.step_count(SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell),
            2
        );
        assert_eq!(
            entities.step_count(SavePostLoadRuntimeExecutionStepStatus::Blocked),
            2
        );
        assert_eq!(
            entities.step_count(SavePostLoadRuntimeExecutionStepStatus::Deferred),
            1
        );
        assert_eq!(entities.total_step_count(), 7);
        assert_eq!(
            custom.step_count(SavePostLoadRuntimeExecutionStepStatus::Executed),
            2
        );
        assert_eq!(
            custom.step_count(SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell),
            1
        );
        assert_eq!(
            custom.step_count(SavePostLoadRuntimeExecutionStepStatus::Failed),
            1
        );
        assert_eq!(custom.total_step_count(), 4);
        assert_eq!(markers.step_count(SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell), 2);
        assert_eq!(markers.total_step_count(), 2);
    }

    #[test]
    fn execute_runtime_apply_allows_activation_with_only_deferred_steps() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let mut execution = observation.execute_runtime_apply();
        execution.executed_steps.clear();
        execution.failed_steps.clear();
        execution.awaiting_world_shell_steps.clear();
        execution.blocked_steps.clear();
        execution.deferred_steps = vec![SavePostLoadRuntimeApplyStep::SkippedEntity {
            entity_index: 1,
        }];

        assert!(execution.can_seed_runtime_apply);
        assert!(execution.world_shell_ready);
        assert!(execution.has_world_shell());
        assert!(execution.executed_steps.is_empty());
        assert!(execution.failed_steps.is_empty());
        assert!(execution.awaiting_world_shell_steps.is_empty());
        assert!(execution.blocked_steps.is_empty());
        assert_eq!(
            execution.deferred_steps,
            vec![SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 }]
        );
        assert_eq!(execution.deferred_step_count(), 1);
        assert_eq!(execution.pending_step_count(), 1);
        assert!(execution.can_activate_live_runtime());
    }

    #[test]
    fn execute_runtime_apply_blocks_activation_on_auxiliary_failures() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.custom_chunks.push(CustomChunkEntry {
            name: observation.custom_chunks[1].name.clone(),
            chunk_len: 1,
            chunk_bytes: vec![0xee],
            chunk_sha256: "mystery-duplicate".to_string(),
            parsed: ParsedCustomChunk::Unknown,
        });

        let execution = observation.execute_runtime_apply();

        assert!(execution.can_seed_runtime_apply);
        assert!(execution.world_shell_ready);
        assert!(execution.has_world_shell());
        assert!(!execution.failed_steps.is_empty());
        assert_eq!(
            execution.failed_steps,
            vec![SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 2 }]
        );
        assert_eq!(
            execution.issues,
            vec![SavePostLoadRuntimeApplyIssue::DuplicateCustomChunkName("mystery".to_string(),)]
        );
        assert!(!execution.can_activate_live_runtime());
    }

    #[test]
    fn execute_runtime_world_semantics_ignores_duplicate_non_world_custom_chunk_failures() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.custom_chunks.push(CustomChunkEntry {
            name: observation.custom_chunks[1].name.clone(),
            chunk_len: 1,
            chunk_bytes: vec![0xee],
            chunk_sha256: "mystery-duplicate".to_string(),
            parsed: ParsedCustomChunk::Unknown,
        });

        let runtime_apply = observation.execute_runtime_apply();
        let execution = observation.execute_runtime_world_semantics();
        let shell = execution.world_shell.as_ref().unwrap();

        assert_eq!(
            runtime_apply.failed_steps,
            vec![SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 2 }]
        );
        assert_eq!(
            runtime_apply.issues,
            vec![SavePostLoadRuntimeApplyIssue::DuplicateCustomChunkName(
                "mystery".to_string(),
            )]
        );
        assert!(!runtime_apply.can_activate_live_runtime());

        assert!(execution.world_shell_ready);
        assert!(execution.has_world_shell());
        assert!(execution.awaiting_world_shell_steps.is_empty());
        assert!(execution.blocked_steps.is_empty());
        assert!(execution.failed_steps.is_empty());
        assert!(execution.issues.is_empty());
        assert!(execution.can_activate_live_runtime());
        assert!(shell.static_fog.is_some());
    }

    #[test]
    fn execute_runtime_apply_records_duplicate_marker_ids_without_overwriting_first_marker() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.markers[1].id = observation.markers[0].id;

        let execution = observation.execute_runtime_apply();
        let shell = execution.world_shell.as_ref().unwrap();

        assert_eq!(
            execution.failed_steps,
            vec![SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 }]
        );
        assert_eq!(
            execution.issues,
            vec![SavePostLoadRuntimeApplyIssue::DuplicateMarkerId(11)]
        );
        assert_eq!(
            shell.owned_step_count(SavePostLoadRuntimeWorldSurfaceKind::Markers),
            1
        );
        assert_eq!(shell.markers.len(), 1);
        assert_eq!(shell.markers_by_id.len(), 1);
        assert!(shell.markers_by_id.contains_key(&11));
    }

    #[test]
    fn execute_runtime_world_semantics_rejects_duplicate_marker_ids_after_world_shell_ready() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.markers[1].id = observation.markers[0].id;

        let execution = observation.execute_runtime_world_semantics();
        let shell = execution.world_shell.as_ref().unwrap();

        assert!(execution.world_shell_ready);
        assert!(execution.has_world_shell());
        assert!(execution.awaiting_world_shell_steps.is_empty());
        assert!(execution.blocked_steps.is_empty());
        assert_eq!(
            execution.failed_steps,
            vec![SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 }]
        );
        assert_eq!(
            execution.issues,
            vec![SavePostLoadRuntimeApplyIssue::DuplicateMarkerId(11)]
        );
        assert!(!execution.can_activate_live_runtime());
        assert_eq!(
            shell.owned_step_count(SavePostLoadRuntimeWorldSurfaceKind::Markers),
            1
        );
        assert_eq!(shell.markers.len(), 1);
        assert_eq!(shell.markers_by_id.len(), 1);
        assert!(shell.markers_by_id.contains_key(&11));
    }

    #[test]
    fn execute_runtime_world_semantics_groups_steps_by_source_region() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.markers[1].id = observation.markers[0].id;

        let execution = observation.execute_runtime_world_semantics();
        let source_regions = execution.source_regions();
        let map = execution.source_region("map").unwrap();
        let entities = execution.source_region("entities").unwrap();
        let markers = execution.source_region("markers").unwrap();
        let custom = execution.source_region("custom").unwrap();

        assert_eq!(source_regions.len(), 4);
        let mut source_region_names = source_regions
            .iter()
            .map(|region| region.source_region_name)
            .collect::<Vec<_>>();
        source_region_names.sort_unstable();
        assert_eq!(source_region_names, vec!["custom", "entities", "map", "markers"]);
        assert_eq!(
            execution.summary_label(),
            format!(
                "shell={} semantics={} exec={} fail={} wait={} block={} defer={} total={} sources={} issues={} live={}",
                bool_label(execution.world_shell_ready),
                bool_label(execution.can_apply_world_semantics()),
                execution.executed_step_count(),
                execution.failed_step_count(),
                execution.awaiting_world_shell_steps.len(),
                execution.blocked_steps.len(),
                execution.deferred_step_count(),
                execution.targeted_step_count(),
                execution.source_regions().len(),
                execution.issues.len(),
                bool_label(execution.can_activate_live_runtime()),
            )
        );
        assert_eq!(
            markers.summary_label(),
            "region=markers exec=1 fail=1 wait=0 block=0 defer=0 total=2"
        );
        assert!(execution
            .detail_label()
            .contains("region=markers exec=1 fail=1 wait=0 block=0 defer=0 total=2"));
        assert_eq!(map.total_step_count(), 2);
        assert_eq!(entities.total_step_count(), 5);
        assert_eq!(markers.total_step_count(), 2);
        assert_eq!(custom.total_step_count(), 1);
        assert_eq!(
            markers.step_count(SavePostLoadRuntimeExecutionStepStatus::Executed),
            1
        );
        assert_eq!(
            markers.step_count(SavePostLoadRuntimeExecutionStepStatus::Failed),
            1
        );
        assert_eq!(markers.total_step_count(), 2);
    }

    #[test]
    fn execute_runtime_world_semantics_keeps_world_shell_overlay_steps_and_ignores_non_world_tail()
    {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        let mut plan = observation.runtime_seed_plan();
        let mut skipped = plan.loadable_entity_seeds[1].clone();
        skipped.entity_index = 99;
        plan.skipped_entity_seeds.push(skipped);

        let execution = plan.execute_runtime_world_semantics();
        let shell = execution.world_shell.as_ref().unwrap();

        assert!(execution.world_shell_ready);
        assert!(execution.can_apply_world_semantics());
        assert!(execution.can_activate_live_runtime());
        assert!(execution.ownership.can_apply_world_semantics());
        assert_eq!(execution.executed_step_count(), 10);
        assert_eq!(execution.failed_step_count(), 0);
        assert_eq!(execution.pending_step_count(), 0);
        assert_eq!(execution.targeted_step_count(), 10);
        assert_eq!(
            execution.executed_steps,
            vec![
                SavePostLoadRuntimeApplyStep::WorldShell,
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 0,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 1,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 0 },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 },
                SavePostLoadRuntimeApplyStep::StaticFog,
                SavePostLoadRuntimeApplyStep::Building { center_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 1 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 },
            ]
        );
        assert!(execution.issues.is_empty());
        assert_eq!(
            execution
                .ownership
                .surface(SavePostLoadRuntimeWorldSurfaceKind::Markers)
                .unwrap()
                .status,
            SavePostLoadRuntimeWorldOwnershipStatus::Owned
        );
        assert_eq!(shell.team_plans.len(), 2);
        assert_eq!(shell.team_plans_by_team.len(), 2);
        assert_eq!(shell.markers.len(), 2);
        assert_eq!(shell.markers_by_id.len(), 2);
        assert!(shell.static_fog.is_some());
        assert_eq!(shell.buildings.len(), 1);
        assert_eq!(shell.buildings_by_center_index.len(), 1);
        assert_eq!(shell.loadable_entities.len(), 3);
        assert_eq!(shell.loadable_entities_by_id.len(), 3);
        assert_eq!(
            shell.owned_step_count(SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities),
            3
        );
        assert_eq!(shell.loadable_entities_by_effective_class_id.len(), 2);
        assert_eq!(
            shell
                .loadable_entities_for_effective_class_id(3)
                .unwrap()
                .iter()
                .map(|seed| seed.activation.entity_id)
                .collect::<Vec<_>>(),
            vec![42, 43]
        );
        assert_eq!(
            shell
                .loadable_entities_for_effective_name("flare")
                .unwrap()
                .iter()
                .map(|seed| seed.activation.entity_id)
                .collect::<Vec<_>>(),
            vec![42, 43]
        );
        assert!(!execution.executed_steps.iter().any(|step| {
            matches!(
                step,
                SavePostLoadRuntimeApplyStep::EntityRemap { .. }
                    | SavePostLoadRuntimeApplyStep::CustomChunk { .. }
                    | SavePostLoadRuntimeApplyStep::SkippedEntity { .. }
            )
        }));
    }

    #[test]
    fn execute_runtime_world_semantics_keeps_world_blockers_without_non_world_pending_steps() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let execution = observation.execute_runtime_world_semantics();

        assert!(!execution.world_shell_ready);
        assert!(!execution.can_apply_world_semantics());
        assert!(!execution.can_activate_live_runtime());
        assert_eq!(
            execution
                .ownership
                .surface(SavePostLoadRuntimeWorldSurfaceKind::WorldShell)
                .unwrap()
                .status,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked
        );
        assert!(!execution.has_world_shell());
        assert!(execution.failed_steps.is_empty());
        assert!(execution.issues.is_empty());
        assert_eq!(
            execution.awaiting_world_shell_steps,
            vec![
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 0,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 1,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 0 },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 },
                SavePostLoadRuntimeApplyStep::StaticFog,
            ]
        );
        assert_eq!(
            execution.blocked_steps,
            vec![
                SavePostLoadRuntimeApplyStep::WorldShell,
                SavePostLoadRuntimeApplyStep::Building { center_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 },
            ]
        );
        assert_eq!(execution.executed_step_count(), 0);
        assert_eq!(execution.pending_step_count(), 9);
        assert_eq!(execution.targeted_step_count(), 9);
    }

    #[test]
    fn world_semantics_activation_rejects_failed_world_steps() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let mut execution = observation.execute_runtime_world_semantics();
        execution
            .failed_steps
            .push(SavePostLoadRuntimeApplyStep::Building { center_index: 0 });

        assert!(execution.world_shell_ready);
        assert!(execution.can_apply_world_semantics());
        assert!(execution.has_world_shell());
        assert!(!execution.failed_steps.is_empty());
        assert!(!execution.can_activate_live_runtime());
    }

    #[test]
    fn world_semantics_activation_rejects_pending_steps() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let mut execution = observation.execute_runtime_world_semantics();
        execution
            .awaiting_world_shell_steps
            .push(SavePostLoadRuntimeApplyStep::TeamPlan {
                group_index: 0,
                plan_index: 0,
            });

        assert!(execution.world_shell_ready);
        assert!(execution.can_apply_world_semantics());
        assert!(execution.has_world_shell());
        assert!(execution.failed_steps.is_empty());
        assert!(!execution.awaiting_world_shell_steps.is_empty());
        assert!(!execution.can_activate_live_runtime());

        let mut execution = observation.execute_runtime_world_semantics();
        execution
            .blocked_steps
            .push(SavePostLoadRuntimeApplyStep::Building { center_index: 0 });

        assert!(execution.world_shell_ready);
        assert!(execution.can_apply_world_semantics());
        assert!(execution.has_world_shell());
        assert!(execution.failed_steps.is_empty());
        assert!(!execution.blocked_steps.is_empty());
        assert!(!execution.can_activate_live_runtime());
    }

    fn make_observation_seedable(observation: &mut crate::SavePostLoadWorldObservation) {
        observation.world_entity_chunks[1].class_id = 3;
        observation.world_entity_chunks[1].custom_name = None;
        observation
            .entity_remap_summary
            .unresolved_effective_names
            .clear();
        observation.entity_summary.loadable_entities = 3;
        observation.entity_summary.skipped_entities = 0;
        observation.entity_summary.builtin_entities = 2;
        observation.entity_summary.custom_entities = 1;
        observation.entity_summary.class_summaries = vec![
            SaveEntityClassSummary {
                class_id: 3,
                kind: SaveEntityClassKind::Builtin,
                resolved_name: "flare".to_string(),
                count: 1,
            },
            SaveEntityClassSummary {
                class_id: 4,
                kind: SaveEntityClassKind::Builtin,
                resolved_name: "mace".to_string(),
                count: 1,
            },
            SaveEntityClassSummary {
                class_id: 255,
                kind: SaveEntityClassKind::Custom,
                resolved_name: "flare".to_string(),
                count: 1,
            },
        ];
        observation.entity_summary.post_load_class_summaries = vec![
            SaveEntityPostLoadClassSummary {
                source_class_ids: vec![3],
                effective_class_id: Some(3),
                kind: SaveEntityPostLoadKind::Builtin,
                resolved_name: "flare".to_string(),
                count: 1,
            },
            SaveEntityPostLoadClassSummary {
                source_class_ids: vec![4],
                effective_class_id: Some(4),
                kind: SaveEntityPostLoadKind::Builtin,
                resolved_name: "mace".to_string(),
                count: 1,
            },
            SaveEntityPostLoadClassSummary {
                source_class_ids: vec![255],
                effective_class_id: Some(3),
                kind: SaveEntityPostLoadKind::RemappedBuiltin,
                resolved_name: "flare".to_string(),
                count: 1,
            },
        ];
    }

    fn test_observation() -> crate::SavePostLoadWorldObservation {
        crate::SavePostLoadWorldObservation {
            save_version: 11,
            content_header: vec![ContentHeaderEntry {
                content_type: 1,
                names: vec!["core-nucleus".to_string()],
            }],
            patches: vec![vec![1, 2, 3]],
            map: SaveMapRegionObservation {
                floor_runs: 1,
                floor_region_bytes: vec![0],
                block_runs: 1,
                block_region_bytes: vec![0],
                world: test_world(),
            },
            entity_remap_entries: vec![
                SaveEntityRemapEntry {
                    custom_id: 255,
                    name: "flare".to_string(),
                },
                SaveEntityRemapEntry {
                    custom_id: 254,
                    name: "mod-unit".to_string(),
                },
            ],
            entity_remap_bytes: vec![1, 2],
            entity_remap_summary: SaveEntityRemapSummary {
                remap_count: 2,
                unique_custom_ids: 2,
                duplicate_custom_ids: Vec::new(),
                unique_names: 2,
                duplicate_names: Vec::new(),
                effective_custom_ids: 1,
                resolved_builtin_custom_ids: vec![255],
                unresolved_effective_names: vec!["mod-unit".to_string()],
            },
            team_plan_groups: vec![
                TeamPlanGroup {
                    team_id: 1,
                    plan_count: 1,
                    plans: vec![TeamPlan {
                        x: 0,
                        y: 0,
                        rotation: 0,
                        block_id: 0x0153,
                        config: TypeIoValue::Integer(9),
                        config_bytes: vec![9],
                        config_sha256: "cfg-0".to_string(),
                    }],
                },
                TeamPlanGroup {
                    team_id: 2,
                    plan_count: 1,
                    plans: vec![TeamPlan {
                        x: 1,
                        y: 1,
                        rotation: 1,
                        block_id: 0x0001,
                        config: TypeIoValue::Null,
                        config_bytes: Vec::new(),
                        config_sha256: "cfg-1".to_string(),
                    }],
                },
            ],
            team_region_bytes: vec![3],
            world_entity_count: 3,
            world_entity_bytes: vec![4],
            world_entity_chunks: vec![
                SaveEntityChunkObservation {
                    chunk_len: 3,
                    chunk_bytes: vec![4, 5, 6],
                    chunk_sha256: "chunk-remap".to_string(),
                    class_id: 255,
                    custom_name: Some("flare".to_string()),
                    entity_id: 42,
                    body_len: 2,
                    body_bytes: vec![5, 6],
                    body_sha256: "entity-remap".to_string(),
                },
                SaveEntityChunkObservation {
                    chunk_len: 3,
                    chunk_bytes: vec![6, 7, 8],
                    chunk_sha256: "chunk-skip".to_string(),
                    class_id: 254,
                    custom_name: Some("mod-unit".to_string()),
                    entity_id: 43,
                    body_len: 2,
                    body_bytes: vec![7, 8],
                    body_sha256: "entity-skip".to_string(),
                },
                SaveEntityChunkObservation {
                    chunk_len: 3,
                    chunk_bytes: vec![8, 9, 10],
                    chunk_sha256: "chunk-builtin".to_string(),
                    class_id: 4,
                    custom_name: None,
                    entity_id: 44,
                    body_len: 2,
                    body_bytes: vec![9, 10],
                    body_sha256: "entity-builtin".to_string(),
                },
            ],
            markers: vec![
                MarkerEntry {
                    id: 11,
                    marker: MarkerModel::Point(PointMarkerModel {
                        class_tag: "Minimap".to_string(),
                        world: true,
                        minimap: true,
                        autoscale: false,
                        draw_layer_bits: 0.0f32.to_bits(),
                        x_bits: 8.0f32.to_bits(),
                        y_bits: 0.0f32.to_bits(),
                        radius_bits: 1.0f32.to_bits(),
                        stroke_bits: 1.0f32.to_bits(),
                        color: Some("ffffff".to_string()),
                    }),
                },
                MarkerEntry {
                    id: 12,
                    marker: MarkerModel::Point(PointMarkerModel {
                        class_tag: "Objective".to_string(),
                        world: true,
                        minimap: false,
                        autoscale: false,
                        draw_layer_bits: 0.0f32.to_bits(),
                        x_bits: 0.0f32.to_bits(),
                        y_bits: 8.0f32.to_bits(),
                        radius_bits: 1.5f32.to_bits(),
                        stroke_bits: 1.0f32.to_bits(),
                        color: Some("00ff00".to_string()),
                    }),
                },
            ],
            marker_region_bytes: b"{markers}".to_vec(),
            custom_chunks: vec![
                CustomChunkEntry {
                    name: "static-fog-data".to_string(),
                    chunk_len: 1,
                    chunk_bytes: vec![7],
                    chunk_sha256: "fog".to_string(),
                    parsed: ParsedCustomChunk::StaticFog(StaticFogChunk {
                        used_teams: 2,
                        width: 2,
                        height: 2,
                        teams: vec![
                            StaticFogTeam {
                                team_id: 1,
                                run_count: 1,
                                rle_bytes: vec![8],
                                discovered: vec![true, false, true, true],
                            },
                            StaticFogTeam {
                                team_id: 2,
                                run_count: 1,
                                rle_bytes: vec![9],
                                discovered: vec![false, true, false, true],
                            },
                        ],
                    }),
                },
                CustomChunkEntry {
                    name: "mystery".to_string(),
                    chunk_len: 2,
                    chunk_bytes: vec![1, 2],
                    chunk_sha256: "mystery".to_string(),
                    parsed: ParsedCustomChunk::Unknown,
                },
            ],
            custom_region_bytes: vec![9],
            entity_summary: SaveEntityPostLoadSummary {
                total_entities: 3,
                unique_entity_ids: 3,
                duplicate_entity_ids: Vec::new(),
                builtin_entities: 1,
                custom_entities: 2,
                unknown_entities: 0,
                class_summaries: Vec::new(),
                loadable_entities: 2,
                skipped_entities: 1,
                post_load_class_summaries: Vec::new(),
            },
        }
    }

    fn test_world() -> WorldModel {
        let floors = vec![1, 1, 1, 1];
        let overlays = vec![0, 0, 0, 0];
        let blocks = vec![0x0153, 0, 0, 0];
        WorldModel {
            width: 2,
            height: 2,
            floors: floors.clone(),
            overlays: overlays.clone(),
            blocks: blocks.clone(),
            tiles: vec![
                TileModel {
                    tile_index: 0,
                    x: 0,
                    y: 0,
                    floor_id: floors[0],
                    overlay_id: overlays[0],
                    block_id: blocks[0],
                    building_center_index: Some(0),
                },
                TileModel {
                    tile_index: 1,
                    x: 1,
                    y: 0,
                    floor_id: floors[1],
                    overlay_id: overlays[1],
                    block_id: blocks[1],
                    building_center_index: None,
                },
                TileModel {
                    tile_index: 2,
                    x: 0,
                    y: 1,
                    floor_id: floors[2],
                    overlay_id: overlays[2],
                    block_id: blocks[2],
                    building_center_index: None,
                },
                TileModel {
                    tile_index: 3,
                    x: 1,
                    y: 1,
                    floor_id: floors[3],
                    overlay_id: overlays[3],
                    block_id: blocks[3],
                    building_center_index: None,
                },
            ],
            building_centers: vec![BuildingCenter {
                tile_index: 0,
                x: 0,
                y: 0,
                block_id: 0x0153,
                chunk_len: 3,
                chunk_bytes: vec![0, 1, 2],
                chunk_sha256: "center".to_string(),
                building: BuildingSnapshot {
                    revision: 0,
                    base_len: 0,
                    base: BuildingBaseSnapshot {
                        health_bits: 1.0f32.to_bits(),
                        rotation: 0,
                        team_id: 1,
                        legacy: false,
                        save_version: None,
                        enabled: None,
                        module_bitmask: None,
                        item_module: None,
                        power_module: None,
                        liquid_module: None,
                        time_scale_bits: None,
                        time_scale_duration_bits: None,
                        last_disabler_pos: None,
                        legacy_consume_connected: None,
                        efficiency: None,
                        optional_efficiency: None,
                        visible_flags: None,
                    },
                    tail_len: 0,
                    tail_bytes: Vec::new(),
                    tail_sha256: "tail".to_string(),
                    parsed_tail: ParsedBuildingTail::Core(CoreTailSnapshot {
                        command_pos_present: false,
                        command_pos_x_bits: 0,
                        command_pos_y_bits: 0,
                    }),
                },
            }],
            data_tiles: 1,
            team_count: 2,
            total_plans: 2,
            team_ids: vec![1, 2],
            team_plan_counts: vec![1, 1],
        }
    }
}
