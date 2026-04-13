fn dedupe_i32(values: &[i32]) -> Vec<i32> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.contains(value) {
            deduped.push(*value);
        }
    }
    deduped
}

fn command_mode_position_target(position: (f32, f32)) -> Option<CommandModePositionTarget> {
    let (x, y) = position;
    if x.is_finite() && y.is_finite() {
        Some(CommandModePositionTarget {
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        })
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandUnitRef {
    pub kind: u8,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandModePositionTarget {
    pub x_bits: u32,
    pub y_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandModeRectProjection {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl CommandModeRectProjection {
    pub fn normalized(self) -> Self {
        Self {
            x0: self.x0.min(self.x1),
            y0: self.y0.min(self.y1),
            x1: self.x0.max(self.x1),
            y1: self.y0.max(self.y1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandModeControlGroupProjection {
    pub index: u8,
    pub unit_ids: Vec<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandModeTargetProjection {
    pub build_target: Option<i32>,
    pub unit_target: Option<CommandUnitRef>,
    pub position_target: Option<CommandModePositionTarget>,
    pub rect_target: Option<CommandModeRectProjection>,
}

impl CommandModeTargetProjection {
    pub fn is_empty(self) -> bool {
        self.build_target.is_none()
            && self.unit_target.is_none()
            && self.position_target.is_none()
            && self.rect_target.is_none()
    }

    pub fn summary_label(self) -> String {
        if self.is_empty() {
            return "none".to_string();
        }

        let mut parts = Vec::new();
        if self.build_target.is_some() {
            parts.push("build");
        }
        if self.unit_target.is_some() {
            parts.push("unit");
        }
        if self.position_target.is_some() {
            parts.push("position");
        }
        if self.rect_target.is_some() {
            parts.push("rect");
        }

        parts.join("+")
    }

    pub fn detail_label(self) -> String {
        if self.is_empty() {
            return "none".to_string();
        }

        let mut parts = Vec::new();
        if let Some(build_target) = self.build_target {
            parts.push(format!("build={build_target}"));
        }
        if let Some(unit_target) = self.unit_target {
            parts.push(format!("unit={}:{}", unit_target.kind, unit_target.value));
        }
        if let Some(position_target) = self.position_target {
            parts.push(format!(
                "position={},{}",
                f32::from_bits(position_target.x_bits),
                f32::from_bits(position_target.y_bits)
            ));
        }
        if let Some(rect_target) = self.rect_target {
            parts.push(format!(
                "rect={},{},{}:{}",
                rect_target.x0, rect_target.y0, rect_target.x1, rect_target.y1
            ));
        }

        parts.join(" ")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandModeCommandSelection {
    pub command_id: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandModeStanceSelection {
    pub stance_id: Option<u8>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandModeRecentControlGroupOperation {
    Bind,
    Recall,
    Clear,
}

impl CommandModeRecentControlGroupOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bind => "group-bind",
            Self::Recall => "group-recall",
            Self::Clear => "group-clear",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandModeSelectionOp {
    Replace,
    Add,
    Toggle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandModeProjection {
    pub active: bool,
    pub selected_units: Vec<i32>,
    pub command_buildings: Vec<i32>,
    pub command_rect: Option<CommandModeRectProjection>,
    pub control_groups: Vec<CommandModeControlGroupProjection>,
    pub last_control_group_operation: Option<CommandModeRecentControlGroupOperation>,
    pub last_target: Option<CommandModeTargetProjection>,
    pub last_command_selection: Option<CommandModeCommandSelection>,
    pub last_stance_selection: Option<CommandModeStanceSelection>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandModeProjectionSummary {
    pub active: bool,
    pub selected_unit_count: usize,
    pub command_building_count: usize,
    pub control_group_count: usize,
    pub has_command_rect: bool,
    pub recent_control_group_operation: Option<CommandModeRecentControlGroupOperation>,
    pub has_recent_target: bool,
    pub has_recent_command_selection: bool,
    pub has_recent_stance_selection: bool,
}

impl CommandModeProjectionSummary {
    pub fn is_empty(self) -> bool {
        !self.active
            && self.selected_unit_count == 0
            && self.command_building_count == 0
            && self.control_group_count == 0
            && !self.has_command_rect
            && self.recent_control_group_operation.is_none()
            && !self.has_recent_target
            && !self.has_recent_command_selection
            && !self.has_recent_stance_selection
    }

    pub fn recent_control_group_label(self) -> &'static str {
        self.recent_control_group_operation
            .map(CommandModeRecentControlGroupOperation::label)
            .unwrap_or("none")
    }

    pub fn recent_selection_label(self) -> &'static str {
        match (
            self.has_recent_target,
            self.has_recent_command_selection,
            self.has_recent_stance_selection,
        ) {
            (false, false, false) => "none",
            (true, false, false) => "target",
            (false, true, false) => "command",
            (false, false, true) => "stance",
            (true, true, false) => "target+command",
            (true, false, true) => "target+stance",
            (false, true, true) => "command+stance",
            (true, true, true) => "target+command+stance",
        }
    }

    pub fn summary_label(self) -> &'static str {
        if self.is_empty() {
            "idle"
        } else {
            let recent_selection_label = self.recent_selection_label();
            if recent_selection_label != "none" {
                recent_selection_label
            } else if self.has_command_rect {
                "rect"
            } else if self.control_group_count > 0 {
                "groups"
            } else if self.command_building_count > 0 {
                "buildings"
            } else if self.selected_unit_count > 0 {
                "units"
            } else if let Some(operation) = self.recent_control_group_operation {
                operation.label()
            } else {
                "active"
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandModeState {
    active: bool,
    selected_units: Vec<i32>,
    command_buildings: Vec<i32>,
    command_rect: Option<CommandModeRectProjection>,
    control_groups: Vec<CommandModeControlGroupProjection>,
    last_control_group_operation: Option<CommandModeRecentControlGroupOperation>,
    last_target: Option<CommandModeTargetProjection>,
    last_command_selection: Option<CommandModeCommandSelection>,
    last_stance_selection: Option<CommandModeStanceSelection>,
}

impl CommandModeState {
    fn clear_recent_target_state(&mut self) {
        self.last_target = None;
        self.last_command_selection = None;
        self.last_stance_selection = None;
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.selected_units.clear();
        self.command_buildings.clear();
        self.command_rect = None;
        self.last_control_group_operation = None;
        self.last_target = None;
        self.last_command_selection = None;
        self.last_stance_selection = None;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn bind_control_group(&mut self, index: u8, unit_ids: &[i32]) {
        let unit_ids = dedupe_i32(unit_ids);
        if unit_ids.is_empty() {
            self.clear_control_group(index);
            return;
        }
        for group in self
            .control_groups
            .iter_mut()
            .filter(|group| group.index != index)
        {
            group.unit_ids.retain(|unit_id| !unit_ids.contains(unit_id));
        }
        self.control_groups
            .retain(|group| group.index == index || !group.unit_ids.is_empty());

        let projection = CommandModeControlGroupProjection { index, unit_ids };
        if let Some(existing) = self
            .control_groups
            .iter_mut()
            .find(|group| group.index == index)
        {
            *existing = projection;
        } else {
            self.control_groups.push(projection);
            self.control_groups.sort_by_key(|group| group.index);
        }
        self.last_control_group_operation = Some(CommandModeRecentControlGroupOperation::Bind);
    }

    pub fn recall_control_group(&mut self, index: u8) -> bool {
        let Some(group) = self
            .control_groups
            .iter()
            .find(|group| group.index == index)
            .cloned()
        else {
            return false;
        };
        self.active = true;
        self.selected_units = group.unit_ids;
        self.command_buildings.clear();
        self.command_rect = None;
        self.clear_recent_target_state();
        self.last_control_group_operation = Some(CommandModeRecentControlGroupOperation::Recall);
        true
    }

    pub fn clear_control_group(&mut self, index: u8) -> bool {
        let before = self.control_groups.len();
        self.control_groups.retain(|group| group.index != index);
        let changed = before != self.control_groups.len();
        if changed {
            self.last_control_group_operation = Some(CommandModeRecentControlGroupOperation::Clear);
        }
        changed
    }

    pub fn set_command_rect(&mut self, rect: Option<CommandModeRectProjection>) {
        self.command_rect = rect;
        if rect.is_some() {
            self.active = true;
        }
    }

    pub fn clear_recent_selections(&mut self) {
        self.clear_recent_target_state();
    }

    pub fn record_unit_clear(&mut self) {
        self.clear();
    }

    pub fn record_unit_control(
        &mut self,
        target: Option<CommandUnitRef>,
        selected_unit_ids: &[i32],
    ) {
        self.active = true;
        self.clear_recent_target_state();
        self.selected_units = dedupe_i32(selected_unit_ids);
        self.command_buildings.clear();
        self.command_rect = None;
        self.last_target = Some(CommandModeTargetProjection {
            unit_target: target,
            ..CommandModeTargetProjection::default()
        });
    }

    pub fn record_unit_building_control_select(
        &mut self,
        target: Option<CommandUnitRef>,
        selected_unit_ids: &[i32],
        build_pos: Option<i32>,
    ) {
        self.active = true;
        self.clear_recent_target_state();
        self.selected_units = dedupe_i32(selected_unit_ids);
        self.command_buildings = build_pos.into_iter().collect();
        self.command_rect = None;
        self.last_target = Some(CommandModeTargetProjection {
            build_target: build_pos,
            unit_target: target,
            ..CommandModeTargetProjection::default()
        });
    }

    pub fn record_building_control_select(&mut self, build_pos: Option<i32>) {
        self.active = true;
        self.clear_recent_target_state();
        self.selected_units.clear();
        self.command_buildings = build_pos.into_iter().collect();
        self.command_rect = None;
        self.last_target = Some(CommandModeTargetProjection {
            build_target: build_pos,
            ..CommandModeTargetProjection::default()
        });
    }

    pub fn select_unit_target(
        &mut self,
        target: Option<CommandUnitRef>,
        selected_unit_ids: &[i32],
        op: CommandModeSelectionOp,
    ) {
        self.active = true;
        self.clear_recent_target_state();
        self.selected_units = merge_selected_units(&self.selected_units, selected_unit_ids, op);
        self.command_buildings.clear();
        self.command_rect = None;
        self.last_target = Some(CommandModeTargetProjection {
            unit_target: target,
            ..CommandModeTargetProjection::default()
        });
    }

    pub fn select_units_rect(
        &mut self,
        rect: CommandModeRectProjection,
        selected_unit_ids: &[i32],
        op: CommandModeSelectionOp,
    ) {
        let rect = rect.normalized();
        self.active = true;
        self.clear_recent_target_state();
        self.selected_units = merge_selected_units(&self.selected_units, selected_unit_ids, op);
        self.command_buildings.clear();
        self.command_rect = Some(rect);
        self.last_target = Some(CommandModeTargetProjection {
            rect_target: Some(rect),
            ..CommandModeTargetProjection::default()
        });
    }

    pub fn record_command_building(&mut self, buildings: &[i32], position: (f32, f32)) {
        self.active = true;
        self.clear_recent_target_state();
        self.selected_units.clear();
        self.command_buildings = dedupe_i32(buildings);
        self.command_rect = None;
        self.last_target = Some(CommandModeTargetProjection {
            position_target: command_mode_position_target(position),
            ..CommandModeTargetProjection::default()
        });
    }

    pub fn record_command_units(
        &mut self,
        unit_ids: &[i32],
        build_target: Option<i32>,
        unit_target: Option<CommandUnitRef>,
        pos_target: Option<(f32, f32)>,
    ) {
        self.active = true;
        self.clear_recent_target_state();
        self.selected_units = dedupe_i32(unit_ids);
        self.command_buildings.clear();
        self.command_rect = None;
        self.last_target = Some(CommandModeTargetProjection {
            build_target,
            unit_target,
            position_target: pos_target.and_then(command_mode_position_target),
            rect_target: None,
        });
    }

    pub fn record_command_target(
        &mut self,
        build_target: Option<i32>,
        unit_target: Option<CommandUnitRef>,
        pos_target: Option<(f32, f32)>,
    ) {
        self.active = true;
        self.clear_recent_target_state();
        self.command_rect = None;
        self.last_target = Some(CommandModeTargetProjection {
            build_target,
            unit_target,
            position_target: pos_target.and_then(command_mode_position_target),
            rect_target: None,
        });
    }

    pub fn record_set_unit_command(&mut self, unit_ids: &[i32], command_id: Option<u8>) {
        self.active = true;
        self.selected_units = dedupe_i32(unit_ids);
        self.last_command_selection = Some(CommandModeCommandSelection { command_id });
    }

    pub fn record_set_unit_stance(
        &mut self,
        unit_ids: &[i32],
        stance_id: Option<u8>,
        enabled: bool,
    ) {
        self.active = true;
        self.selected_units = dedupe_i32(unit_ids);
        self.last_stance_selection = Some(CommandModeStanceSelection { stance_id, enabled });
    }

    pub fn projection(&self) -> CommandModeProjection {
        CommandModeProjection {
            active: self.active,
            selected_units: self.selected_units.clone(),
            command_buildings: self.command_buildings.clone(),
            command_rect: self.command_rect,
            control_groups: self.control_groups.clone(),
            last_control_group_operation: self.last_control_group_operation,
            last_target: self.last_target,
            last_command_selection: self.last_command_selection,
            last_stance_selection: self.last_stance_selection,
        }
    }
}

impl CommandModeProjection {
    pub fn is_empty(&self) -> bool {
        self.summary().is_empty()
    }

    pub fn summary(&self) -> CommandModeProjectionSummary {
        CommandModeProjectionSummary {
            active: self.active,
            selected_unit_count: self.selected_units.len(),
            command_building_count: self.command_buildings.len(),
            control_group_count: self.control_groups.len(),
            has_command_rect: self.command_rect.is_some(),
            recent_control_group_operation: self.last_control_group_operation,
            has_recent_target: self.last_target.is_some_and(|target| !target.is_empty()),
            has_recent_command_selection: self.last_command_selection.is_some(),
            has_recent_stance_selection: self.last_stance_selection.is_some(),
        }
    }

    pub fn summary_label(&self) -> &'static str {
        self.summary().summary_label()
    }

    pub fn recent_selection_label(&self) -> &'static str {
        self.summary().recent_selection_label()
    }

    pub fn recent_control_group_label(&self) -> &'static str {
        self.summary().recent_control_group_label()
    }
}

pub fn merge_selected_units(
    current_unit_ids: &[i32],
    incoming_unit_ids: &[i32],
    op: CommandModeSelectionOp,
) -> Vec<i32> {
    let current = dedupe_i32(current_unit_ids);
    let incoming = dedupe_i32(incoming_unit_ids);
    match op {
        CommandModeSelectionOp::Replace => incoming,
        CommandModeSelectionOp::Add => {
            let mut merged = current;
            for unit_id in incoming {
                if !merged.contains(&unit_id) {
                    merged.push(unit_id);
                }
            }
            merged
        }
        CommandModeSelectionOp::Toggle => {
            let mut merged = current;
            for unit_id in incoming {
                if let Some(index) = merged.iter().position(|existing| *existing == unit_id) {
                    merged.remove(index);
                } else {
                    merged.push(unit_id);
                }
            }
            merged
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(kind: u8, value: i32) -> CommandUnitRef {
        CommandUnitRef { kind, value }
    }

    #[test]
    fn target_projection_is_empty_only_without_any_target() {
        assert!(CommandModeTargetProjection::default().is_empty());
        assert_eq!(CommandModeTargetProjection::default().summary_label(), "none");
        assert_eq!(CommandModeTargetProjection::default().detail_label(), "none");
        assert!(!CommandModeTargetProjection {
            build_target: Some(7),
            ..CommandModeTargetProjection::default()
        }
        .is_empty());
        assert!(!CommandModeTargetProjection {
            unit_target: Some(unit(1, 9)),
            ..CommandModeTargetProjection::default()
        }
        .is_empty());
        assert!(!CommandModeTargetProjection {
            position_target: Some(CommandModePositionTarget {
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
            }),
            ..CommandModeTargetProjection::default()
        }
        .is_empty());
        assert!(!CommandModeTargetProjection {
            rect_target: Some(CommandModeRectProjection {
                x0: 1,
                y0: 2,
                x1: 3,
                y1: 4,
            }),
            ..CommandModeTargetProjection::default()
        }
        .is_empty());
    }

    #[test]
    fn target_projection_summary_label_reports_empty_mixed_and_full_targets() {
        assert_eq!(CommandModeTargetProjection::default().summary_label(), "none");

        let mixed = CommandModeTargetProjection {
            build_target: Some(7),
            unit_target: Some(unit(1, 9)),
            position_target: Some(CommandModePositionTarget {
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.5f32.to_bits(),
            }),
            rect_target: None,
        };
        assert_eq!(mixed.summary_label(), "build+unit+position");
        assert!(mixed.detail_label().contains("build=7"));
        assert!(mixed.detail_label().contains("unit=1:9"));
        assert!(mixed.detail_label().contains("position=1,2.5"));

        let full = CommandModeTargetProjection {
            rect_target: Some(CommandModeRectProjection {
                x0: -1,
                y0: 2,
                x1: 3,
                y1: 4,
            }),
            ..mixed
        };
        assert_eq!(full.summary_label(), "build+unit+position+rect");
        assert!(full.detail_label().contains("rect=-1,2,3:4"));
    }

    #[test]
    fn command_mode_projection_summary_reports_empty_and_recent_selection_state() {
        let empty = CommandModeProjection::default();

        assert!(empty.is_empty());
        assert_eq!(
            empty.summary(),
            CommandModeProjectionSummary {
                active: false,
                selected_unit_count: 0,
                command_building_count: 0,
                control_group_count: 0,
                has_command_rect: false,
                recent_control_group_operation: None,
                has_recent_target: false,
                has_recent_command_selection: false,
                has_recent_stance_selection: false,
            }
        );
        assert_eq!(empty.summary_label(), "idle");
        assert_eq!(empty.recent_selection_label(), "none");
        assert_eq!(empty.recent_control_group_label(), "none");

        let mut state = CommandModeState::default();
        state.bind_control_group(2, &[9, 9, 7]);
        state.record_command_units(&[11, 22, 11], Some(7), Some(unit(2, 33)), Some((1.0, 2.0)));
        state.record_set_unit_command(&[11, 22, 11], Some(5));
        state.record_set_unit_stance(&[11, 22, 11], None, true);

        let summary = state.projection().summary();
        assert_eq!(summary.active, true);
        assert_eq!(summary.selected_unit_count, 2);
        assert_eq!(summary.command_building_count, 0);
        assert_eq!(summary.control_group_count, 1);
        assert_eq!(summary.has_command_rect, false);
        assert_eq!(
            summary.recent_control_group_operation,
            Some(CommandModeRecentControlGroupOperation::Bind)
        );
        assert_eq!(summary.has_recent_target, true);
        assert_eq!(summary.has_recent_command_selection, true);
        assert_eq!(summary.has_recent_stance_selection, true);
        assert_eq!(summary.summary_label(), "target+command+stance");
        assert_eq!(summary.recent_selection_label(), "target+command+stance");
        assert_eq!(summary.recent_control_group_label(), "group-bind");
    }

    #[test]
    fn command_mode_state_projection_tracks_selection_and_recent_command_state() {
        let mut state = CommandModeState::default();
        state.bind_control_group(2, &[9, 9, 7]);
        state.set_command_rect(Some(CommandModeRectProjection {
            x0: 1,
            y0: 2,
            x1: 3,
            y1: 4,
        }));
        state.record_command_units(&[11, 22, 11], Some(7), Some(unit(2, 33)), Some((1.0, 2.0)));
        state.record_set_unit_command(&[11, 22, 11], Some(5));
        state.record_set_unit_stance(&[11, 22, 11], None, true);

        assert_eq!(
            state.projection(),
            CommandModeProjection {
                active: true,
                selected_units: vec![11, 22],
                command_buildings: Vec::new(),
                command_rect: None,
                control_groups: vec![CommandModeControlGroupProjection {
                    index: 2,
                    unit_ids: vec![9, 7],
                }],
                last_control_group_operation: Some(
                    CommandModeRecentControlGroupOperation::Bind,
                ),
                last_target: Some(CommandModeTargetProjection {
                    build_target: Some(7),
                    unit_target: Some(unit(2, 33)),
                    position_target: Some(CommandModePositionTarget {
                        x_bits: 1.0f32.to_bits(),
                        y_bits: 2.0f32.to_bits(),
                    }),
                    rect_target: None,
                }),
                last_command_selection: Some(CommandModeCommandSelection {
                    command_id: Some(5),
                }),
                last_stance_selection: Some(CommandModeStanceSelection {
                    stance_id: None,
                    enabled: true,
                }),
            }
        );
    }

    #[test]
    fn record_command_positions_reject_or_canonicalize_non_finite_coordinates() {
        let mut state = CommandModeState::default();

        state.record_command_building(&[3, 3, 4], (f32::NAN, 2.0));
        assert_eq!(state.projection().command_buildings, vec![3, 4]);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                position_target: None,
                ..CommandModeTargetProjection::default()
            })
        );

        state.record_command_units(
            &[8, 8, 9],
            Some(1),
            Some(unit(7, 12)),
            Some((f32::INFINITY, 5.0)),
        );
        assert_eq!(state.projection().selected_units, vec![8, 9]);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: Some(1),
                unit_target: Some(unit(7, 12)),
                position_target: None,
                rect_target: None,
            })
        );
    }

    #[test]
    fn building_selection_helpers_accept_none_and_control_groups_survive_clear() {
        let mut state = CommandModeState::default();
        state.bind_control_group(1, &[44, 55]);
        state.select_unit_target(Some(unit(2, 11)), &[11, 22], CommandModeSelectionOp::Replace);
        assert_eq!(state.projection().selected_units, vec![11, 22]);

        state.record_building_control_select(Some(90));
        assert!(state.projection().selected_units.is_empty());
        assert_eq!(state.projection().command_buildings, vec![90]);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: Some(90),
                unit_target: None,
                position_target: None,
                rect_target: None,
            })
        );

        state.record_unit_building_control_select(Some(unit(2, 44)), &[44], Some(90));
        assert_eq!(state.projection().command_buildings, vec![90]);

        state.record_building_control_select(None);
        assert!(state.projection().selected_units.is_empty());
        assert!(state.projection().command_buildings.is_empty());
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: None,
                position_target: None,
                rect_target: None,
            })
        );

        assert!(state.recall_control_group(1));
        assert_eq!(state.projection().selected_units, vec![44, 55]);

        state.record_unit_clear();
        assert!(!state.is_active());
        assert!(state.projection().selected_units.is_empty());
        assert_eq!(
            state.projection().control_groups,
            vec![CommandModeControlGroupProjection {
                index: 1,
                unit_ids: vec![44, 55],
            }]
        );
    }

    #[test]
    fn recalling_control_group_clears_stale_command_targets_and_rects() {
        let mut state = CommandModeState::default();
        state.bind_control_group(2, &[77, 88]);
        state.record_building_control_select(Some(90));
        state.set_command_rect(Some(CommandModeRectProjection {
            x0: -2,
            y0: 3,
            x1: 4,
            y1: 9,
        }));
        state.record_set_unit_command(&[11], Some(5));
        state.record_set_unit_stance(&[11], Some(7), true);

        assert!(state.recall_control_group(2));

        assert_eq!(state.projection().selected_units, vec![77, 88]);
        assert!(state.projection().command_buildings.is_empty());
        assert_eq!(state.projection().command_rect, None);
        assert_eq!(state.projection().last_target, None);
        assert_eq!(state.projection().last_control_group_operation, Some(CommandModeRecentControlGroupOperation::Recall));
        assert_eq!(state.projection().last_command_selection, None);
        assert_eq!(state.projection().last_stance_selection, None);
    }

    #[test]
    fn command_entries_clear_stale_opposite_selection_state() {
        let mut state = CommandModeState::default();
        state.select_unit_target(Some(unit(2, 11)), &[11, 22], CommandModeSelectionOp::Replace);
        state.record_set_unit_command(&[11, 22], Some(5));
        state.record_set_unit_stance(&[11, 22], Some(7), true);
        state.record_command_building(&[90, 91, 90], (3.0, 4.0));

        assert!(state.projection().selected_units.is_empty());
        assert_eq!(state.projection().command_buildings, vec![90, 91]);
        assert_eq!(state.projection().last_command_selection, None);
        assert_eq!(state.projection().last_stance_selection, None);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                position_target: Some(CommandModePositionTarget {
                    x_bits: 3.0f32.to_bits(),
                    y_bits: 4.0f32.to_bits(),
                }),
                ..CommandModeTargetProjection::default()
            })
        );

        state.record_set_unit_command(&[7, 8], Some(9));
        state.record_set_unit_stance(&[7, 8], Some(3), false);
        state.record_command_units(&[7, 8, 7], Some(12), Some(unit(1, 44)), Some((5.0, 6.0)));

        assert_eq!(state.projection().selected_units, vec![7, 8]);
        assert!(state.projection().command_buildings.is_empty());
        assert_eq!(state.projection().last_command_selection, None);
        assert_eq!(state.projection().last_stance_selection, None);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: Some(12),
                unit_target: Some(unit(1, 44)),
                position_target: Some(CommandModePositionTarget {
                    x_bits: 5.0f32.to_bits(),
                    y_bits: 6.0f32.to_bits(),
                }),
                rect_target: None,
            })
        );
    }

    #[test]
    fn record_command_target_preserves_existing_selection_state() {
        let mut state = CommandModeState::default();
        state.record_building_control_select(Some(404));
        state.record_set_unit_command(&[77, 88, 77], Some(7));

        state.record_command_target(Some(808), Some(unit(2, 909)), Some((1.5, -2.25)));

        assert_eq!(state.projection().selected_units, vec![77, 88]);
        assert_eq!(state.projection().command_buildings, vec![404]);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: Some(808),
                unit_target: Some(unit(2, 909)),
                position_target: Some(CommandModePositionTarget {
                    x_bits: 1.5f32.to_bits(),
                    y_bits: (-2.25f32).to_bits(),
                }),
                rect_target: None,
            })
        );
        assert_eq!(state.projection().last_command_selection, None);
        assert_eq!(state.projection().last_stance_selection, None);
    }

    #[test]
    fn command_mode_projection_summary_tracks_recent_control_group_operations() {
        let mut bind_state = CommandModeState::default();
        bind_state.bind_control_group(3, &[10, 20, 20]);
        let bind_summary = bind_state.projection().summary();
        assert_eq!(
            bind_summary.recent_control_group_operation,
            Some(CommandModeRecentControlGroupOperation::Bind)
        );
        assert_eq!(bind_summary.recent_control_group_label(), "group-bind");
        assert_eq!(bind_summary.summary_label(), "groups");

        let mut recall_state = CommandModeState::default();
        recall_state.bind_control_group(4, &[30, 40]);
        assert!(recall_state.recall_control_group(4));
        let recall_summary = recall_state.projection().summary();
        assert_eq!(
            recall_summary.recent_control_group_operation,
            Some(CommandModeRecentControlGroupOperation::Recall)
        );
        assert_eq!(recall_summary.recent_control_group_label(), "group-recall");
        assert_eq!(recall_summary.summary_label(), "groups");

        let mut clear_state = CommandModeState::default();
        clear_state.bind_control_group(5, &[50, 60]);
        assert!(clear_state.clear_control_group(5));
        let clear_summary = clear_state.projection().summary();
        assert_eq!(
            clear_summary.recent_control_group_operation,
            Some(CommandModeRecentControlGroupOperation::Clear)
        );
        assert_eq!(clear_summary.recent_control_group_label(), "group-clear");
        assert_eq!(clear_summary.summary_label(), "group-clear");
    }

    #[test]
    fn bind_control_group_moves_units_across_groups_exclusively() {
        let mut state = CommandModeState::default();
        state.bind_control_group(1, &[10, 20]);
        state.bind_control_group(2, &[20, 30, 30]);

        assert_eq!(
            state.projection().control_groups,
            vec![
                CommandModeControlGroupProjection {
                    index: 1,
                    unit_ids: vec![10],
                },
                CommandModeControlGroupProjection {
                    index: 2,
                    unit_ids: vec![20, 30],
                },
            ]
        );

        state.bind_control_group(3, &[10, 20, 30]);

        assert_eq!(
            state.projection().control_groups,
            vec![CommandModeControlGroupProjection {
                index: 3,
                unit_ids: vec![10, 20, 30],
            }]
        );
    }

    #[test]
    fn merge_selected_units_applies_replace_add_and_toggle_stably() {
        assert_eq!(
            merge_selected_units(&[11, 22], &[33, 22, 44], CommandModeSelectionOp::Replace),
            vec![33, 22, 44]
        );
        assert_eq!(
            merge_selected_units(&[11, 22], &[22, 33, 11, 44], CommandModeSelectionOp::Add),
            vec![11, 22, 33, 44]
        );
        assert_eq!(
            merge_selected_units(&[11, 22, 33], &[22, 44, 11], CommandModeSelectionOp::Toggle),
            vec![33, 44]
        );
    }

    #[test]
    fn select_units_rect_normalizes_bounds_and_merges_additively() {
        let mut state = CommandModeState::default();
        state.select_unit_target(
            Some(unit(2, 11)),
            &[11, 22],
            CommandModeSelectionOp::Replace,
        );

        state.select_units_rect(
            CommandModeRectProjection {
                x0: 8,
                y0: 9,
                x1: 3,
                y1: 4,
            },
            &[22, 33, 44],
            CommandModeSelectionOp::Add,
        );

        assert_eq!(state.projection().selected_units, vec![11, 22, 33, 44]);
        assert_eq!(
            state.projection().command_rect,
            Some(CommandModeRectProjection {
                x0: 3,
                y0: 4,
                x1: 8,
                y1: 9,
            })
        );
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                rect_target: Some(CommandModeRectProjection {
                    x0: 3,
                    y0: 4,
                    x1: 8,
                    y1: 9,
                }),
                ..CommandModeTargetProjection::default()
            })
        );
    }

    #[test]
    fn select_unit_target_toggle_updates_selection_without_touching_control_groups() {
        let mut state = CommandModeState::default();
        state.bind_control_group(4, &[10, 20]);
        state.record_set_unit_command(&[10, 20], Some(4));
        state.record_set_unit_stance(&[10, 20], Some(8), true);
        state.select_unit_target(
            Some(unit(2, 10)),
            &[10, 20],
            CommandModeSelectionOp::Replace,
        );
        state.select_unit_target(Some(unit(2, 30)), &[20, 30], CommandModeSelectionOp::Toggle);

        assert_eq!(state.projection().selected_units, vec![10, 30]);
        assert_eq!(state.projection().command_rect, None);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                unit_target: Some(unit(2, 30)),
                ..CommandModeTargetProjection::default()
            })
        );
        assert_eq!(state.projection().last_command_selection, None);
        assert_eq!(state.projection().last_stance_selection, None);
        assert_eq!(
            state.projection().control_groups,
            vec![CommandModeControlGroupProjection {
                index: 4,
                unit_ids: vec![10, 20],
            }]
        );
    }
}
