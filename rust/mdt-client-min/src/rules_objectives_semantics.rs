#[derive(Debug, Default, Clone, PartialEq)]
pub struct RulesProjection {
    pub infinite_resources: Option<bool>,
    pub wave_timer: Option<bool>,
    pub wave_sending: Option<bool>,
    pub waves: Option<bool>,
    pub wait_enemies: Option<bool>,
    pub pvp: Option<bool>,
    pub can_game_over: Option<bool>,
    pub core_capture: Option<bool>,
    pub reactor_explosions: Option<bool>,
    pub schematics_allowed: Option<bool>,
    pub fire: Option<bool>,
    pub unit_ammo: Option<bool>,
    pub ghost_blocks: Option<bool>,
    pub logic_unit_control: Option<bool>,
    pub logic_unit_build: Option<bool>,
    pub logic_unit_deconstruct: Option<bool>,
    pub block_whitelist: Option<bool>,
    pub unit_whitelist: Option<bool>,
    pub win_wave: Option<i32>,
    pub unit_cap: Option<i32>,
    pub disable_unit_cap: Option<bool>,
    pub default_team_id: Option<i32>,
    pub wave_team_id: Option<i32>,
    pub wave_spacing: Option<f64>,
    pub initial_wave_spacing: Option<f64>,
    pub attack_mode: Option<bool>,
    pub build_cost_multiplier: Option<f64>,
    pub build_speed_multiplier: Option<f64>,
    pub unit_build_speed_multiplier: Option<f64>,
    pub unit_cost_multiplier: Option<f64>,
    pub unit_damage_multiplier: Option<f64>,
    pub unit_health_multiplier: Option<f64>,
    pub unit_crash_damage_multiplier: Option<f64>,
    pub unit_mine_speed_multiplier: Option<f64>,
    pub block_health_multiplier: Option<f64>,
    pub block_damage_multiplier: Option<f64>,
    pub deconstruct_refund_multiplier: Option<f64>,
    pub objective_timer_multiplier: Option<f64>,
    pub enemy_core_build_radius: Option<f64>,
    pub drop_zone_radius: Option<f64>,
    pub replaced_from_set_rules_count: u64,
    pub applied_set_rule_patch_count: u64,
    pub unknown_set_rule_patch_count: u64,
    pub ignored_set_rule_patch_count: u64,
    pub last_applied_set_rule_patch_name: Option<String>,
    pub last_applied_set_rule_patch_json_data: Option<String>,
    pub last_unknown_set_rule_patch_name: Option<String>,
    pub last_ignored_set_rule_patch_name: Option<String>,
}

impl RulesProjection {
    pub fn apply_set_rules_json(&mut self, json_data: &str) {
        self.replaced_from_set_rules_count = self.replaced_from_set_rules_count.saturating_add(1);
        self.infinite_resources = object_field_bool(json_data, "infiniteResources");
        self.wave_timer = object_field_bool(json_data, "waveTimer");
        self.wave_sending = object_field_bool(json_data, "waveSending");
        self.waves = object_field_bool(json_data, "waves");
        self.wait_enemies = object_field_bool(json_data, "waitEnemies");
        self.pvp = object_field_bool(json_data, "pvp");
        self.can_game_over = object_field_bool(json_data, "canGameOver");
        self.core_capture = object_field_bool(json_data, "coreCapture");
        self.reactor_explosions = object_field_bool(json_data, "reactorExplosions");
        self.schematics_allowed = object_field_bool(json_data, "schematicsAllowed");
        self.fire = object_field_bool(json_data, "fire");
        self.unit_ammo = object_field_bool(json_data, "unitAmmo");
        self.ghost_blocks = object_field_bool(json_data, "ghostBlocks");
        self.logic_unit_control = object_field_bool(json_data, "logicUnitControl");
        self.logic_unit_build = object_field_bool(json_data, "logicUnitBuild");
        self.logic_unit_deconstruct = object_field_bool(json_data, "logicUnitDeconstruct");
        self.block_whitelist = object_field_bool(json_data, "blockWhitelist");
        self.unit_whitelist = object_field_bool(json_data, "unitWhitelist");
        self.win_wave = object_field_i32(json_data, "winWave");
        self.unit_cap = object_field_i32(json_data, "unitCap");
        self.disable_unit_cap = object_field_bool(json_data, "disableUnitCap");
        self.default_team_id = object_field_i32(json_data, "defaultTeam");
        self.wave_team_id = object_field_i32(json_data, "waveTeam");
        self.wave_spacing = object_field_f64(json_data, "waveSpacing");
        self.initial_wave_spacing = object_field_f64(json_data, "initialWaveSpacing");
        self.attack_mode = object_field_bool(json_data, "attackMode");
        self.build_cost_multiplier = object_field_f64(json_data, "buildCostMultiplier");
        self.build_speed_multiplier = object_field_f64(json_data, "buildSpeedMultiplier");
        self.unit_build_speed_multiplier = object_field_f64(json_data, "unitBuildSpeedMultiplier");
        self.unit_cost_multiplier = object_field_f64(json_data, "unitCostMultiplier");
        self.unit_damage_multiplier = object_field_f64(json_data, "unitDamageMultiplier");
        self.unit_health_multiplier = object_field_f64(json_data, "unitHealthMultiplier");
        self.unit_crash_damage_multiplier =
            object_field_f64(json_data, "unitCrashDamageMultiplier");
        self.unit_mine_speed_multiplier = object_field_f64(json_data, "unitMineSpeedMultiplier");
        self.block_health_multiplier = object_field_f64(json_data, "blockHealthMultiplier");
        self.block_damage_multiplier = object_field_f64(json_data, "blockDamageMultiplier");
        self.deconstruct_refund_multiplier =
            object_field_f64(json_data, "deconstructRefundMultiplier");
        self.objective_timer_multiplier = object_field_f64(json_data, "objectiveTimerMultiplier");
        self.enemy_core_build_radius = object_field_f64(json_data, "enemyCoreBuildRadius");
        self.drop_zone_radius = object_field_f64(json_data, "dropZoneRadius");
    }

    pub fn apply_set_rule_patch(&mut self, rule: &str, json_data: &str) {
        self.applied_set_rule_patch_count = self.applied_set_rule_patch_count.saturating_add(1);
        self.last_applied_set_rule_patch_name = Some(rule.to_string());
        self.last_applied_set_rule_patch_json_data = Some(json_data.to_string());

        let applied = match rule {
            "infiniteResources" => parse_json_bool_literal(json_data).map(|value| {
                self.infinite_resources = Some(value);
            }),
            "waveTimer" => parse_json_bool_literal(json_data).map(|value| {
                self.wave_timer = Some(value);
            }),
            "waveSending" => parse_json_bool_literal(json_data).map(|value| {
                self.wave_sending = Some(value);
            }),
            "waves" => parse_json_bool_literal(json_data).map(|value| {
                self.waves = Some(value);
            }),
            "waitEnemies" => parse_json_bool_literal(json_data).map(|value| {
                self.wait_enemies = Some(value);
            }),
            "pvp" => parse_json_bool_literal(json_data).map(|value| {
                self.pvp = Some(value);
            }),
            "canGameOver" => parse_json_bool_literal(json_data).map(|value| {
                self.can_game_over = Some(value);
            }),
            "coreCapture" => parse_json_bool_literal(json_data).map(|value| {
                self.core_capture = Some(value);
            }),
            "reactorExplosions" => parse_json_bool_literal(json_data).map(|value| {
                self.reactor_explosions = Some(value);
            }),
            "schematicsAllowed" => parse_json_bool_literal(json_data).map(|value| {
                self.schematics_allowed = Some(value);
            }),
            "fire" => parse_json_bool_literal(json_data).map(|value| {
                self.fire = Some(value);
            }),
            "unitAmmo" => parse_json_bool_literal(json_data).map(|value| {
                self.unit_ammo = Some(value);
            }),
            "ghostBlocks" => parse_json_bool_literal(json_data).map(|value| {
                self.ghost_blocks = Some(value);
            }),
            "logicUnitControl" => parse_json_bool_literal(json_data).map(|value| {
                self.logic_unit_control = Some(value);
            }),
            "logicUnitBuild" => parse_json_bool_literal(json_data).map(|value| {
                self.logic_unit_build = Some(value);
            }),
            "logicUnitDeconstruct" => parse_json_bool_literal(json_data).map(|value| {
                self.logic_unit_deconstruct = Some(value);
            }),
            "blockWhitelist" => parse_json_bool_literal(json_data).map(|value| {
                self.block_whitelist = Some(value);
            }),
            "unitWhitelist" => parse_json_bool_literal(json_data).map(|value| {
                self.unit_whitelist = Some(value);
            }),
            "winWave" => parse_json_i32_literal(json_data).map(|value| {
                self.win_wave = Some(value);
            }),
            "unitCap" => parse_json_i32_literal(json_data).map(|value| {
                self.unit_cap = Some(value);
            }),
            "disableUnitCap" => parse_json_bool_literal(json_data).map(|value| {
                self.disable_unit_cap = Some(value);
            }),
            "defaultTeam" => parse_json_i32_literal(json_data).map(|value| {
                self.default_team_id = Some(value);
            }),
            "waveTeam" => parse_json_i32_literal(json_data).map(|value| {
                self.wave_team_id = Some(value);
            }),
            "waveSpacing" => parse_json_f64_literal(json_data).map(|value| {
                self.wave_spacing = Some(value);
            }),
            "initialWaveSpacing" => parse_json_f64_literal(json_data).map(|value| {
                self.initial_wave_spacing = Some(value);
            }),
            "attackMode" => parse_json_bool_literal(json_data).map(|value| {
                self.attack_mode = Some(value);
            }),
            "buildCostMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.build_cost_multiplier = Some(value);
            }),
            "buildSpeedMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.build_speed_multiplier = Some(value);
            }),
            "unitBuildSpeedMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.unit_build_speed_multiplier = Some(value);
            }),
            "unitCostMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.unit_cost_multiplier = Some(value);
            }),
            "unitDamageMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.unit_damage_multiplier = Some(value);
            }),
            "unitHealthMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.unit_health_multiplier = Some(value);
            }),
            "unitCrashDamageMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.unit_crash_damage_multiplier = Some(value);
            }),
            "unitMineSpeedMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.unit_mine_speed_multiplier = Some(value);
            }),
            "blockHealthMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.block_health_multiplier = Some(value);
            }),
            "blockDamageMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.block_damage_multiplier = Some(value);
            }),
            "deconstructRefundMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.deconstruct_refund_multiplier = Some(value);
            }),
            "objectiveTimerMultiplier" => parse_json_f64_literal(json_data).map(|value| {
                self.objective_timer_multiplier = Some(value);
            }),
            "enemyCoreBuildRadius" => parse_json_f64_literal(json_data).map(|value| {
                self.enemy_core_build_radius = Some(value);
            }),
            "dropZoneRadius" => parse_json_f64_literal(json_data).map(|value| {
                self.drop_zone_radius = Some(value);
            }),
            _ => None,
        };

        match (rule, applied) {
            (_, Some(())) => {}
            (
                "infiniteResources"
                | "waveTimer"
                | "waveSending"
                | "waves"
                | "waitEnemies"
                | "pvp"
                | "canGameOver"
                | "coreCapture"
                | "reactorExplosions"
                | "schematicsAllowed"
                | "fire"
                | "unitAmmo"
                | "ghostBlocks"
                | "logicUnitControl"
                | "logicUnitBuild"
                | "logicUnitDeconstruct"
                | "blockWhitelist"
                | "unitWhitelist"
                | "winWave"
                | "unitCap"
                | "disableUnitCap"
                | "defaultTeam"
                | "waveTeam"
                | "waveSpacing"
                | "initialWaveSpacing"
                | "attackMode"
                | "buildCostMultiplier"
                | "buildSpeedMultiplier"
                | "unitBuildSpeedMultiplier"
                | "unitCostMultiplier"
                | "unitDamageMultiplier"
                | "unitHealthMultiplier"
                | "unitCrashDamageMultiplier"
                | "unitMineSpeedMultiplier"
                | "blockHealthMultiplier"
                | "blockDamageMultiplier"
                | "deconstructRefundMultiplier"
                | "objectiveTimerMultiplier"
                | "enemyCoreBuildRadius"
                | "dropZoneRadius",
                None,
            ) => {
                self.ignored_set_rule_patch_count =
                    self.ignored_set_rule_patch_count.saturating_add(1);
                self.last_ignored_set_rule_patch_name = Some(rule.to_string());
            }
            _ => {
                self.unknown_set_rule_patch_count =
                    self.unknown_set_rule_patch_count.saturating_add(1);
                self.last_unknown_set_rule_patch_name = Some(rule.to_string());
            }
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ObjectiveProjection {
    pub objective_type: Option<String>,
    pub completed: bool,
    pub hidden: bool,
    pub target_name: Option<String>,
    pub amount: Option<i32>,
    pub text: Option<String>,
    pub details: Option<String>,
    pub completion_logic_code: Option<String>,
    pub flag: Option<String>,
    pub team: Option<String>,
    pub team_id: Option<i32>,
    pub has_position: bool,
    pub positions_count: Option<usize>,
    pub flags_added_count: Option<usize>,
    pub flags_removed_count: Option<usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ObjectivesProjection {
    pub objectives: Vec<ObjectiveProjection>,
    pub replaced_from_set_objectives_count: u64,
    pub cleared_count: u64,
    pub complete_by_index_count: u64,
    pub complete_out_of_range_count: u64,
    pub last_completed_index: Option<i32>,
}

impl ObjectivesProjection {
    pub fn replace_from_json(&mut self, json_data: &str) {
        self.replaced_from_set_objectives_count =
            self.replaced_from_set_objectives_count.saturating_add(1);
        self.objectives = parse_objective_list(json_data);
        self.last_completed_index = None;
    }

    pub fn clear(&mut self) {
        self.cleared_count = self.cleared_count.saturating_add(1);
        self.objectives.clear();
        self.last_completed_index = None;
    }

    pub fn complete_by_index(&mut self, index: i32) {
        self.complete_by_index_count = self.complete_by_index_count.saturating_add(1);
        self.last_completed_index = Some(index);
        if let Ok(index_usize) = usize::try_from(index) {
            if let Some(objective) = self.objectives.get_mut(index_usize) {
                objective.completed = true;
                return;
            }
        }
        self.complete_out_of_range_count = self.complete_out_of_range_count.saturating_add(1);
    }
}

fn parse_objective_list(json_data: &str) -> Vec<ObjectiveProjection> {
    array_value_slices(json_data)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            let target_name = ["content", "item", "unit", "block"]
                .into_iter()
                .find_map(|field| object_field_string(entry, field));
            ObjectiveProjection {
                objective_type: object_field_string(entry, "type"),
                completed: object_field_bool(entry, "completed")
                    .or_else(|| object_field_bool(entry, "complete"))
                    .unwrap_or(false),
                hidden: object_field_bool(entry, "hidden").unwrap_or(false),
                target_name,
                amount: object_field_i32(entry, "amount")
                    .or_else(|| object_field_i32(entry, "count")),
                text: object_field_string(entry, "text"),
                details: object_field_string(entry, "details"),
                completion_logic_code: object_field_string(entry, "completionLogicCode"),
                flag: object_field_string(entry, "flag"),
                team: object_field_string(entry, "team"),
                team_id: object_field_i32(entry, "team"),
                has_position: object_field_value(entry, "pos").is_some(),
                positions_count: object_field_array_len(entry, "positions"),
                flags_added_count: object_field_array_len(entry, "flagsAdded"),
                flags_removed_count: object_field_array_len(entry, "flagsRemoved"),
            }
        })
        .collect()
}

fn object_field_bool(json: &str, key: &str) -> Option<bool> {
    object_field_value(json, key).and_then(parse_json_bool_literal)
}

fn object_field_f64(json: &str, key: &str) -> Option<f64> {
    object_field_value(json, key).and_then(parse_json_f64_literal)
}

fn object_field_i32(json: &str, key: &str) -> Option<i32> {
    object_field_value(json, key).and_then(parse_json_i32_literal)
}

fn object_field_array_len(json: &str, key: &str) -> Option<usize> {
    object_field_value(json, key)
        .and_then(array_value_slices)
        .map(|values| values.len())
}

fn object_field_string(json: &str, key: &str) -> Option<String> {
    object_field_value(json, key).and_then(parse_json_string_literal)
}

fn parse_json_bool_literal(raw: &str) -> Option<bool> {
    match raw.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_json_f64_literal(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok()
}

fn parse_json_i32_literal(raw: &str) -> Option<i32> {
    raw.trim().parse::<i32>().ok()
}

fn parse_json_string_literal(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if bytes.first().copied() != Some(b'"') {
        return None;
    }
    let end = parse_json_string_end(bytes, 0)?;
    if end != bytes.len() {
        return None;
    }
    Some(trimmed[1..end - 1].to_string())
}

fn object_field_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let bytes = json.as_bytes();
    let mut cursor = skip_ws(bytes, 0);
    if bytes.get(cursor).copied() != Some(b'{') {
        return None;
    }
    cursor += 1;
    loop {
        cursor = skip_ws(bytes, cursor);
        if bytes.get(cursor).copied() == Some(b'}') {
            return None;
        }
        let (field, next_cursor) = parse_json_string_raw(json, cursor)?;
        cursor = skip_ws(bytes, next_cursor);
        if bytes.get(cursor).copied() != Some(b':') {
            return None;
        }
        cursor = skip_ws(bytes, cursor + 1);
        let value_start = cursor;
        let value_end = skip_json_value(bytes, cursor)?;
        if field == key {
            return Some(json[value_start..value_end].trim());
        }
        cursor = skip_ws(bytes, value_end);
        match bytes.get(cursor).copied() {
            Some(b',') => cursor += 1,
            Some(b'}') => return None,
            _ => return None,
        }
    }
}

fn array_value_slices(json: &str) -> Option<Vec<&str>> {
    let bytes = json.as_bytes();
    let mut cursor = skip_ws(bytes, 0);
    if bytes.get(cursor).copied() != Some(b'[') {
        return None;
    }
    cursor += 1;
    let mut values = Vec::new();
    loop {
        cursor = skip_ws(bytes, cursor);
        if bytes.get(cursor).copied() == Some(b']') {
            return Some(values);
        }
        let value_start = cursor;
        let value_end = skip_json_value(bytes, cursor)?;
        values.push(json[value_start..value_end].trim());
        cursor = skip_ws(bytes, value_end);
        match bytes.get(cursor).copied() {
            Some(b',') => cursor += 1,
            Some(b']') => return Some(values),
            _ => return None,
        }
    }
}

fn parse_json_string_raw<'a>(json: &'a str, start: usize) -> Option<(&'a str, usize)> {
    let bytes = json.as_bytes();
    if bytes.get(start).copied() != Some(b'"') {
        return None;
    }
    let end = parse_json_string_end(bytes, start)?;
    Some((&json[start + 1..end - 1], end))
}

fn parse_json_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start).copied() != Some(b'"') {
        return None;
    }
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => {
                cursor += 1;
                if cursor >= bytes.len() {
                    return None;
                }
                cursor += 1;
            }
            b'"' => return Some(cursor + 1),
            _ => cursor += 1,
        }
    }
    None
}

fn skip_json_value(bytes: &[u8], start: usize) -> Option<usize> {
    let cursor = skip_ws(bytes, start);
    match bytes.get(cursor).copied() {
        Some(b'"') => parse_json_string_end(bytes, cursor),
        Some(b'{') => skip_json_container(bytes, cursor, b'{', b'}'),
        Some(b'[') => skip_json_container(bytes, cursor, b'[', b']'),
        Some(b't') => match_literal(bytes, cursor, b"true"),
        Some(b'f') => match_literal(bytes, cursor, b"false"),
        Some(b'n') => match_literal(bytes, cursor, b"null"),
        Some(b'-' | b'0'..=b'9') => parse_json_number_end(bytes, cursor),
        _ => None,
    }
}

fn skip_json_container(bytes: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    if bytes.get(start).copied() != Some(open) {
        return None;
    }
    let mut depth = 0usize;
    let mut cursor = start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => {
                cursor = parse_json_string_end(bytes, cursor)?;
            }
            byte if byte == open => {
                depth += 1;
                cursor += 1;
            }
            byte if byte == close => {
                depth = depth.saturating_sub(1);
                cursor += 1;
                if depth == 0 {
                    return Some(cursor);
                }
            }
            _ => cursor += 1,
        }
    }
    None
}

fn parse_json_number_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    if bytes.get(cursor).copied() == Some(b'-') {
        cursor += 1;
    }

    let mut integer_digits = 0usize;
    while matches!(bytes.get(cursor).copied(), Some(b'0'..=b'9')) {
        cursor += 1;
        integer_digits += 1;
    }
    if integer_digits == 0 {
        return None;
    }

    if bytes.get(cursor).copied() == Some(b'.') {
        cursor += 1;
        let mut fractional_digits = 0usize;
        while matches!(bytes.get(cursor).copied(), Some(b'0'..=b'9')) {
            cursor += 1;
            fractional_digits += 1;
        }
        if fractional_digits == 0 {
            return None;
        }
    }

    if matches!(bytes.get(cursor).copied(), Some(b'e') | Some(b'E')) {
        cursor += 1;
        if matches!(bytes.get(cursor).copied(), Some(b'+') | Some(b'-')) {
            cursor += 1;
        }
        let mut exponent_digits = 0usize;
        while matches!(bytes.get(cursor).copied(), Some(b'0'..=b'9')) {
            cursor += 1;
            exponent_digits += 1;
        }
        if exponent_digits == 0 {
            return None;
        }
    }

    Some(cursor)
}

fn match_literal(bytes: &[u8], start: usize, literal: &[u8]) -> Option<usize> {
    let end = start + literal.len();
    if bytes.get(start..end) == Some(literal) {
        Some(end)
    } else {
        None
    }
}

fn skip_ws(bytes: &[u8], mut cursor: usize) -> usize {
    while matches!(
        bytes.get(cursor).copied(),
        Some(b' ') | Some(b'\n') | Some(b'\r') | Some(b'\t')
    ) {
        cursor += 1;
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::{ObjectivesProjection, RulesProjection};

    fn apply_mixed_update_sequence(
        rules: &mut RulesProjection,
        objectives: &mut ObjectivesProjection,
    ) {
        rules
            .apply_set_rules_json(r#"{"waves":true,"pvp":false,"unitCap":10,"waveSpacing":120.0}"#);
        rules.apply_set_rule_patch("waves", "false");
        rules.apply_set_rule_patch("pvp", "true");
        rules.apply_set_rule_patch("wave", r#"{"spacing":60}"#);
        rules.apply_set_rule_patch("buildSpeedMultiplier", r#"{"bad":1}"#);

        objectives.replace_from_json(
            r#"[{"type":"Research","content":"router","completed":false},{"type":"Item","item":"lead","amount":20,"completed":false}]"#,
        );
        objectives.complete_by_index(1);
        objectives.clear();
        objectives.complete_by_index(0);
        objectives.replace_from_json(
            r#"[{"type":"Flag","flag":"boss","text":"@boss","completed":false}]"#,
        );
        objectives.complete_by_index(0);

        rules.apply_set_rules_json(r#"{"waveTimer":true}"#);
        rules.apply_set_rule_patch("pvp", "false");
    }

    #[test]
    fn rules_projection_replaces_whitelisted_fields_from_set_rules() {
        let mut projection = RulesProjection::default();
        projection.apply_set_rules_json(
            r#"{"infiniteResources":true,"waveTimer":false,"waveSending":true,"waves":true,"waitEnemies":true,"pvp":true,"canGameOver":false,"coreCapture":true,"reactorExplosions":false,"schematicsAllowed":true,"fire":false,"unitAmmo":true,"ghostBlocks":false,"logicUnitControl":true,"logicUnitBuild":false,"logicUnitDeconstruct":true,"blockWhitelist":true,"unitWhitelist":false,"winWave":25,"unitCap":42,"disableUnitCap":true,"defaultTeam":1,"waveTeam":2,"waveSpacing":120.5,"initialWaveSpacing":240.5,"attackMode":false,"buildCostMultiplier":0.8,"buildSpeedMultiplier":1.5,"unitBuildSpeedMultiplier":1.25,"unitCostMultiplier":0.9,"unitDamageMultiplier":0.75,"unitHealthMultiplier":1.1,"unitCrashDamageMultiplier":1.3,"unitMineSpeedMultiplier":1.4,"blockHealthMultiplier":1.6,"blockDamageMultiplier":0.95,"deconstructRefundMultiplier":0.55,"objectiveTimerMultiplier":2.0,"enemyCoreBuildRadius":600.0,"dropZoneRadius":128.0,"unknown":7}"#,
        );

        assert_eq!(projection.infinite_resources, Some(true));
        assert_eq!(projection.wave_timer, Some(false));
        assert_eq!(projection.wave_sending, Some(true));
        assert_eq!(projection.waves, Some(true));
        assert_eq!(projection.wait_enemies, Some(true));
        assert_eq!(projection.pvp, Some(true));
        assert_eq!(projection.can_game_over, Some(false));
        assert_eq!(projection.core_capture, Some(true));
        assert_eq!(projection.reactor_explosions, Some(false));
        assert_eq!(projection.schematics_allowed, Some(true));
        assert_eq!(projection.fire, Some(false));
        assert_eq!(projection.unit_ammo, Some(true));
        assert_eq!(projection.ghost_blocks, Some(false));
        assert_eq!(projection.logic_unit_control, Some(true));
        assert_eq!(projection.logic_unit_build, Some(false));
        assert_eq!(projection.logic_unit_deconstruct, Some(true));
        assert_eq!(projection.block_whitelist, Some(true));
        assert_eq!(projection.unit_whitelist, Some(false));
        assert_eq!(projection.win_wave, Some(25));
        assert_eq!(projection.unit_cap, Some(42));
        assert_eq!(projection.disable_unit_cap, Some(true));
        assert_eq!(projection.default_team_id, Some(1));
        assert_eq!(projection.wave_team_id, Some(2));
        assert_eq!(projection.wave_spacing, Some(120.5));
        assert_eq!(projection.initial_wave_spacing, Some(240.5));
        assert_eq!(projection.attack_mode, Some(false));
        assert_eq!(projection.build_cost_multiplier, Some(0.8));
        assert_eq!(projection.build_speed_multiplier, Some(1.5));
        assert_eq!(projection.unit_build_speed_multiplier, Some(1.25));
        assert_eq!(projection.unit_cost_multiplier, Some(0.9));
        assert_eq!(projection.unit_damage_multiplier, Some(0.75));
        assert_eq!(projection.unit_health_multiplier, Some(1.1));
        assert_eq!(projection.unit_crash_damage_multiplier, Some(1.3));
        assert_eq!(projection.unit_mine_speed_multiplier, Some(1.4));
        assert_eq!(projection.block_health_multiplier, Some(1.6));
        assert_eq!(projection.block_damage_multiplier, Some(0.95));
        assert_eq!(projection.deconstruct_refund_multiplier, Some(0.55));
        assert_eq!(projection.objective_timer_multiplier, Some(2.0));
        assert_eq!(projection.enemy_core_build_radius, Some(600.0));
        assert_eq!(projection.drop_zone_radius, Some(128.0));
        assert_eq!(projection.replaced_from_set_rules_count, 1);

        projection.apply_set_rules_json(r#"{"waves":false}"#);

        assert_eq!(projection.infinite_resources, None);
        assert_eq!(projection.wave_timer, None);
        assert_eq!(projection.wave_sending, None);
        assert_eq!(projection.waves, Some(false));
        assert_eq!(projection.wait_enemies, None);
        assert_eq!(projection.pvp, None);
        assert_eq!(projection.can_game_over, None);
        assert_eq!(projection.core_capture, None);
        assert_eq!(projection.reactor_explosions, None);
        assert_eq!(projection.schematics_allowed, None);
        assert_eq!(projection.fire, None);
        assert_eq!(projection.unit_ammo, None);
        assert_eq!(projection.ghost_blocks, None);
        assert_eq!(projection.logic_unit_control, None);
        assert_eq!(projection.logic_unit_build, None);
        assert_eq!(projection.logic_unit_deconstruct, None);
        assert_eq!(projection.block_whitelist, None);
        assert_eq!(projection.unit_whitelist, None);
        assert_eq!(projection.win_wave, None);
        assert_eq!(projection.unit_cap, None);
        assert_eq!(projection.disable_unit_cap, None);
        assert_eq!(projection.default_team_id, None);
        assert_eq!(projection.wave_team_id, None);
        assert_eq!(projection.wave_spacing, None);
        assert_eq!(projection.initial_wave_spacing, None);
        assert_eq!(projection.attack_mode, None);
        assert_eq!(projection.build_cost_multiplier, None);
        assert_eq!(projection.build_speed_multiplier, None);
        assert_eq!(projection.unit_build_speed_multiplier, None);
        assert_eq!(projection.unit_cost_multiplier, None);
        assert_eq!(projection.unit_damage_multiplier, None);
        assert_eq!(projection.unit_health_multiplier, None);
        assert_eq!(projection.unit_crash_damage_multiplier, None);
        assert_eq!(projection.unit_mine_speed_multiplier, None);
        assert_eq!(projection.block_health_multiplier, None);
        assert_eq!(projection.block_damage_multiplier, None);
        assert_eq!(projection.deconstruct_refund_multiplier, None);
        assert_eq!(projection.objective_timer_multiplier, None);
        assert_eq!(projection.enemy_core_build_radius, None);
        assert_eq!(projection.drop_zone_radius, None);
        assert_eq!(projection.replaced_from_set_rules_count, 2);
    }

    #[test]
    fn rules_projection_applies_known_set_rule_patch_and_records_unknown() {
        let mut projection = RulesProjection::default();
        projection.apply_set_rule_patch("infiniteResources", "true");
        projection.apply_set_rule_patch("waitEnemies", "false");
        projection.apply_set_rule_patch("pvp", "true");
        projection.apply_set_rule_patch("canGameOver", "false");
        projection.apply_set_rule_patch("coreCapture", "true");
        projection.apply_set_rule_patch("reactorExplosions", "false");
        projection.apply_set_rule_patch("schematicsAllowed", "true");
        projection.apply_set_rule_patch("fire", "false");
        projection.apply_set_rule_patch("unitAmmo", "true");
        projection.apply_set_rule_patch("ghostBlocks", "false");
        projection.apply_set_rule_patch("logicUnitControl", "true");
        projection.apply_set_rule_patch("logicUnitBuild", "false");
        projection.apply_set_rule_patch("logicUnitDeconstruct", "true");
        projection.apply_set_rule_patch("blockWhitelist", "true");
        projection.apply_set_rule_patch("unitWhitelist", "false");
        projection.apply_set_rule_patch("winWave", "15");
        projection.apply_set_rule_patch("unitCap", "70");
        projection.apply_set_rule_patch("disableUnitCap", "true");
        projection.apply_set_rule_patch("defaultTeam", "1");
        projection.apply_set_rule_patch("waveTeam", "2");
        projection.apply_set_rule_patch("waveSpacing", "30.0");
        projection.apply_set_rule_patch("initialWaveSpacing", "90.0");
        projection.apply_set_rule_patch("buildCostMultiplier", "0.7");
        projection.apply_set_rule_patch("unitBuildSpeedMultiplier", "1.4");
        projection.apply_set_rule_patch("unitCostMultiplier", "0.8");
        projection.apply_set_rule_patch("dropZoneRadius", "96");
        projection.apply_set_rule_patch("waves", "true");
        projection.apply_set_rule_patch("unitHealthMultiplier", "1.1");
        projection.apply_set_rule_patch("unitCrashDamageMultiplier", "1.2");
        projection.apply_set_rule_patch("unitMineSpeedMultiplier", "1.3");
        projection.apply_set_rule_patch("blockHealthMultiplier", "1.4");
        projection.apply_set_rule_patch("blockDamageMultiplier", "0.9");
        projection.apply_set_rule_patch("deconstructRefundMultiplier", "0.6");
        projection.apply_set_rule_patch("wave", r#"{"spacing":60}"#);
        projection.apply_set_rule_patch("buildSpeedMultiplier", r#"{"bad":1}"#);

        assert_eq!(projection.infinite_resources, Some(true));
        assert_eq!(projection.wait_enemies, Some(false));
        assert_eq!(projection.pvp, Some(true));
        assert_eq!(projection.can_game_over, Some(false));
        assert_eq!(projection.core_capture, Some(true));
        assert_eq!(projection.reactor_explosions, Some(false));
        assert_eq!(projection.schematics_allowed, Some(true));
        assert_eq!(projection.fire, Some(false));
        assert_eq!(projection.unit_ammo, Some(true));
        assert_eq!(projection.ghost_blocks, Some(false));
        assert_eq!(projection.logic_unit_control, Some(true));
        assert_eq!(projection.logic_unit_build, Some(false));
        assert_eq!(projection.logic_unit_deconstruct, Some(true));
        assert_eq!(projection.block_whitelist, Some(true));
        assert_eq!(projection.unit_whitelist, Some(false));
        assert_eq!(projection.win_wave, Some(15));
        assert_eq!(projection.unit_cap, Some(70));
        assert_eq!(projection.disable_unit_cap, Some(true));
        assert_eq!(projection.default_team_id, Some(1));
        assert_eq!(projection.wave_team_id, Some(2));
        assert_eq!(projection.wave_spacing, Some(30.0));
        assert_eq!(projection.initial_wave_spacing, Some(90.0));
        assert_eq!(projection.build_cost_multiplier, Some(0.7));
        assert_eq!(projection.unit_build_speed_multiplier, Some(1.4));
        assert_eq!(projection.unit_cost_multiplier, Some(0.8));
        assert_eq!(projection.drop_zone_radius, Some(96.0));
        assert_eq!(projection.waves, Some(true));
        assert_eq!(projection.unit_health_multiplier, Some(1.1));
        assert_eq!(projection.unit_crash_damage_multiplier, Some(1.2));
        assert_eq!(projection.unit_mine_speed_multiplier, Some(1.3));
        assert_eq!(projection.block_health_multiplier, Some(1.4));
        assert_eq!(projection.block_damage_multiplier, Some(0.9));
        assert_eq!(projection.deconstruct_refund_multiplier, Some(0.6));
        assert_eq!(projection.applied_set_rule_patch_count, 35);
        assert_eq!(projection.unknown_set_rule_patch_count, 1);
        assert_eq!(
            projection.last_unknown_set_rule_patch_name.as_deref(),
            Some("wave")
        );
        assert_eq!(projection.ignored_set_rule_patch_count, 1);
        assert_eq!(
            projection.last_ignored_set_rule_patch_name.as_deref(),
            Some("buildSpeedMultiplier")
        );
    }

    #[test]
    fn objectives_projection_replaces_completes_and_clears() {
        let mut projection = ObjectivesProjection::default();
        projection.replace_from_json(
            r#"[{"type":"Timer","text":"@wave","count":90,"hidden":true,"details":"Hold out","completionLogicCode":"sensor @unit @dead"},{"type":"Item","item":"lead","amount":15,"flagsAdded":["a","b"],"flagsRemoved":["c"]},{"type":"DestroyBlocks","block":"router","team":2,"positions":[{"x":1,"y":2},{"x":3,"y":4}],"details":"Break routers"},{"type":"DestroyBlock","block":"duo","team":"malis","pos":{"x":8,"y":9},"completed":true,"completionLogicCode":"print 1"},{"type":"Flag","flag":"boss","text":"@boss"},42]"#,
        );

        assert_eq!(projection.objectives.len(), 6);
        assert_eq!(
            projection.objectives[0].objective_type.as_deref(),
            Some("Timer")
        );
        assert_eq!(
            projection.objectives[1].objective_type.as_deref(),
            Some("Item")
        );
        assert_eq!(projection.objectives[0].amount, Some(90));
        assert_eq!(projection.objectives[0].text.as_deref(), Some("@wave"));
        assert!(projection.objectives[0].hidden);
        assert_eq!(
            projection.objectives[0].details.as_deref(),
            Some("Hold out")
        );
        assert_eq!(
            projection.objectives[0].completion_logic_code.as_deref(),
            Some("sensor @unit @dead")
        );
        assert_eq!(
            projection.objectives[1].target_name.as_deref(),
            Some("lead")
        );
        assert_eq!(projection.objectives[1].amount, Some(15));
        assert_eq!(projection.objectives[1].flags_added_count, Some(2));
        assert_eq!(projection.objectives[1].flags_removed_count, Some(1));
        assert_eq!(
            projection.objectives[2].target_name.as_deref(),
            Some("router")
        );
        assert_eq!(projection.objectives[2].team, None);
        assert_eq!(projection.objectives[2].team_id, Some(2));
        assert_eq!(
            projection.objectives[2].details.as_deref(),
            Some("Break routers")
        );
        assert_eq!(projection.objectives[2].positions_count, Some(2));
        assert_eq!(projection.objectives[3].target_name.as_deref(), Some("duo"));
        assert_eq!(projection.objectives[3].team.as_deref(), Some("malis"));
        assert_eq!(projection.objectives[3].team_id, None);
        assert!(projection.objectives[3].has_position);
        assert_eq!(
            projection.objectives[3].completion_logic_code.as_deref(),
            Some("print 1")
        );
        assert_eq!(projection.objectives[4].flag.as_deref(), Some("boss"));
        assert_eq!(projection.objectives[4].text.as_deref(), Some("@boss"));
        assert!(!projection.objectives[0].completed);
        assert!(projection.objectives[3].completed);
        assert_eq!(projection.replaced_from_set_objectives_count, 1);

        projection.complete_by_index(0);
        projection.complete_by_index(9);
        assert!(projection.objectives[0].completed);
        assert_eq!(projection.complete_by_index_count, 2);
        assert_eq!(projection.complete_out_of_range_count, 1);
        assert_eq!(projection.last_completed_index, Some(9));

        projection.clear();
        assert!(projection.objectives.is_empty());
        assert_eq!(projection.cleared_count, 1);
        assert_eq!(projection.last_completed_index, None);
    }

    #[test]
    fn mixed_rules_objectives_sequence_is_deterministic() {
        let mut rules_a = RulesProjection::default();
        let mut objectives_a = ObjectivesProjection::default();
        let mut rules_b = RulesProjection::default();
        let mut objectives_b = ObjectivesProjection::default();

        apply_mixed_update_sequence(&mut rules_a, &mut objectives_a);
        apply_mixed_update_sequence(&mut rules_b, &mut objectives_b);

        assert_eq!(rules_a, rules_b);
        assert_eq!(objectives_a, objectives_b);

        assert_eq!(rules_a.replaced_from_set_rules_count, 2);
        assert_eq!(rules_a.applied_set_rule_patch_count, 5);
        assert_eq!(rules_a.unknown_set_rule_patch_count, 1);
        assert_eq!(rules_a.ignored_set_rule_patch_count, 1);
        assert_eq!(rules_a.wave_timer, Some(true));
        assert_eq!(rules_a.waves, None);
        assert_eq!(rules_a.pvp, Some(false));
        assert_eq!(rules_a.unit_cap, None);
        assert_eq!(rules_a.wave_spacing, None);

        assert_eq!(objectives_a.replaced_from_set_objectives_count, 2);
        assert_eq!(objectives_a.cleared_count, 1);
        assert_eq!(objectives_a.complete_by_index_count, 3);
        assert_eq!(objectives_a.complete_out_of_range_count, 1);
        assert_eq!(objectives_a.last_completed_index, Some(0));
        assert_eq!(objectives_a.objectives.len(), 1);
        assert!(objectives_a.objectives[0].completed);
        assert_eq!(
            objectives_a.objectives[0].objective_type.as_deref(),
            Some("Flag")
        );
    }

    #[test]
    fn mixed_update_order_changes_state_in_expected_direction() {
        let mut rules_set_rule_then_set_rules = RulesProjection::default();
        rules_set_rule_then_set_rules.apply_set_rule_patch("waves", "false");
        rules_set_rule_then_set_rules.apply_set_rules_json(r#"{"waveTimer":true}"#);

        let mut rules_set_rules_then_set_rule = RulesProjection::default();
        rules_set_rules_then_set_rule.apply_set_rules_json(r#"{"waveTimer":true}"#);
        rules_set_rules_then_set_rule.apply_set_rule_patch("waves", "false");

        assert_eq!(rules_set_rule_then_set_rules.wave_timer, Some(true));
        assert_eq!(rules_set_rule_then_set_rules.waves, None);
        assert_eq!(rules_set_rules_then_set_rule.wave_timer, Some(true));
        assert_eq!(rules_set_rules_then_set_rule.waves, Some(false));

        let mut objectives_complete_then_clear = ObjectivesProjection::default();
        objectives_complete_then_clear
            .replace_from_json(r#"[{"type":"Research","content":"router","completed":false}]"#);
        objectives_complete_then_clear.complete_by_index(0);
        objectives_complete_then_clear.clear();

        let mut objectives_clear_then_complete = ObjectivesProjection::default();
        objectives_clear_then_complete
            .replace_from_json(r#"[{"type":"Research","content":"router","completed":false}]"#);
        objectives_clear_then_complete.clear();
        objectives_clear_then_complete.complete_by_index(0);
        objectives_clear_then_complete
            .replace_from_json(r#"[{"type":"Research","content":"router","completed":false}]"#);
        objectives_clear_then_complete.complete_by_index(0);

        assert!(objectives_complete_then_clear.objectives.is_empty());
        assert_eq!(objectives_complete_then_clear.last_completed_index, None);
        assert_eq!(
            objectives_complete_then_clear.complete_out_of_range_count,
            0
        );

        assert_eq!(objectives_clear_then_complete.objectives.len(), 1);
        assert!(objectives_clear_then_complete.objectives[0].completed);
        assert_eq!(objectives_clear_then_complete.last_completed_index, Some(0));
        assert_eq!(
            objectives_clear_then_complete.complete_out_of_range_count,
            1
        );
    }
}
