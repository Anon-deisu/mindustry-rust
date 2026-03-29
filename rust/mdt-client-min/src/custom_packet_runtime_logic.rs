use mdt_typeio::{unpack_point2, TypeIoObject};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LogicValueExtraction<T> {
    pub value: T,
    pub source: &'static str,
}

pub(crate) fn extract_logic_string(value: &TypeIoObject) -> Option<String> {
    match value {
        TypeIoObject::String(Some(text)) => Some(text.clone()),
        TypeIoObject::ObjectArray(_) => find_first_nested_logic_match(
            value,
            |object| matches!(object, TypeIoObject::String(Some(_))),
        )
            .and_then(|matched| match matched.value {
                TypeIoObject::String(Some(text)) => Some(text.clone()),
                _ => None,
            }),
        _ => None,
    }
}

pub(crate) fn extract_logic_world_pos(
    value: &TypeIoObject,
) -> Option<LogicValueExtraction<(f64, f64)>> {
    extract_logic_value(
        value,
        |object| match object {
            TypeIoObject::Point2 { x, y } => Some(LogicValueExtraction {
                value: (*x as f64, *y as f64),
                source: "point2",
            }),
            TypeIoObject::Vec2 { x, y } => Some(LogicValueExtraction {
                value: (*x as f64, *y as f64),
                source: "vec2",
            }),
            TypeIoObject::PackedPoint2Array(values) => values.first().map(|packed| {
                let (x, y) = unpack_point2(*packed);
                LogicValueExtraction {
                    value: (x as f64, y as f64),
                    source: "point2_array_first",
                }
            }),
            TypeIoObject::Vec2Array(values) => values.first().map(|(x, y)| LogicValueExtraction {
                value: (*x as f64, *y as f64),
                source: "vec2_array_first",
            }),
            TypeIoObject::ObjectArray(_) => None,
            _ => None,
        },
        |object| match object {
            TypeIoObject::Point2 { .. } | TypeIoObject::Vec2 { .. } => true,
            TypeIoObject::PackedPoint2Array(values) => !values.is_empty(),
            TypeIoObject::Vec2Array(values) => !values.is_empty(),
            _ => false,
        },
        |object| match object {
            TypeIoObject::Point2 { x, y } => Some(LogicValueExtraction {
                value: (*x as f64, *y as f64),
                source: "point2_nested",
            }),
            TypeIoObject::Vec2 { x, y } => Some(LogicValueExtraction {
                value: (*x as f64, *y as f64),
                source: "vec2_nested",
            }),
            TypeIoObject::PackedPoint2Array(values) => values.first().map(|packed| {
                let (x, y) = unpack_point2(*packed);
                LogicValueExtraction {
                    value: (x as f64, y as f64),
                    source: "point2_array_first_nested",
                }
            }),
            TypeIoObject::Vec2Array(values) => values.first().map(|(x, y)| LogicValueExtraction {
                value: (*x as f64, *y as f64),
                source: "vec2_array_first_nested",
            }),
            _ => None,
        },
    )
}

pub(crate) fn extract_logic_build_pos(
    value: &TypeIoObject,
) -> Option<LogicValueExtraction<i32>> {
    extract_logic_value(
        value,
        |object| match object {
            TypeIoObject::BuildingPos(build_pos) => Some(LogicValueExtraction {
                value: *build_pos,
                source: "building_pos",
            }),
            TypeIoObject::Int(build_pos) => Some(LogicValueExtraction {
                value: *build_pos,
                source: "int",
            }),
            TypeIoObject::Long(build_pos) => i32::try_from(*build_pos).ok().map(|value| {
                LogicValueExtraction {
                    value,
                    source: "long",
                }
            }),
            TypeIoObject::ObjectArray(_) => None,
            _ => None,
        },
        |object| {
            matches!(
                object,
                TypeIoObject::BuildingPos(_)
                    | TypeIoObject::Int(_)
                    | TypeIoObject::Long(_)
            )
        },
        |object| match object {
            TypeIoObject::BuildingPos(build_pos) => Some(LogicValueExtraction {
                value: *build_pos,
                source: "building_pos_nested",
            }),
            TypeIoObject::Int(build_pos) => Some(LogicValueExtraction {
                value: *build_pos,
                source: "int_nested",
            }),
            TypeIoObject::Long(build_pos) => i32::try_from(*build_pos).ok().map(|value| {
                LogicValueExtraction {
                    value,
                    source: "long_nested",
                }
            }),
            _ => None,
        },
    )
}

pub(crate) fn extract_logic_unit_id(
    value: &TypeIoObject,
) -> Option<LogicValueExtraction<i32>> {
    extract_logic_value(
        value,
        |object| match object {
            TypeIoObject::UnitId(unit_id) => Some(LogicValueExtraction {
                value: *unit_id,
                source: "unit_id",
            }),
            TypeIoObject::Int(unit_id) => Some(LogicValueExtraction {
                value: *unit_id,
                source: "int",
            }),
            TypeIoObject::Long(unit_id) => i32::try_from(*unit_id).ok().map(|value| {
                LogicValueExtraction {
                    value,
                    source: "long",
                }
            }),
            TypeIoObject::ObjectArray(_) => None,
            _ => None,
        },
        |object| {
            matches!(
                object,
                TypeIoObject::UnitId(_) | TypeIoObject::Int(_) | TypeIoObject::Long(_)
            )
        },
        |object| match object {
            TypeIoObject::UnitId(unit_id) => Some(LogicValueExtraction {
                value: *unit_id,
                source: "unit_id_nested",
            }),
            TypeIoObject::Int(unit_id) => Some(LogicValueExtraction {
                value: *unit_id,
                source: "int_nested",
            }),
            TypeIoObject::Long(unit_id) => i32::try_from(*unit_id).ok().map(|value| {
                LogicValueExtraction {
                    value,
                    source: "long_nested",
                }
            }),
            _ => None,
        },
    )
}

pub(crate) fn extract_logic_team(value: &TypeIoObject) -> Option<LogicValueExtraction<u8>> {
    extract_logic_value(
        value,
        |object| match object {
            TypeIoObject::Team(team) => Some(LogicValueExtraction {
                value: *team,
                source: "team",
            }),
            TypeIoObject::Int(team) => u8::try_from(*team).ok().map(|value| LogicValueExtraction {
                value,
                source: "int",
            }),
            TypeIoObject::Long(team) => u8::try_from(*team).ok().map(|value| {
                LogicValueExtraction {
                    value,
                    source: "long",
                }
            }),
            TypeIoObject::ObjectArray(_) => None,
            _ => None,
        },
        |object| {
            matches!(
                object,
                TypeIoObject::Team(_) | TypeIoObject::Int(_) | TypeIoObject::Long(_)
            )
        },
        |object| match object {
            TypeIoObject::Team(team) => Some(LogicValueExtraction {
                value: *team,
                source: "team_nested",
            }),
            TypeIoObject::Int(team) => u8::try_from(*team).ok().map(|value| LogicValueExtraction {
                value,
                source: "int_nested",
            }),
            TypeIoObject::Long(team) => u8::try_from(*team).ok().map(|value| {
                LogicValueExtraction {
                    value,
                    source: "long_nested",
                }
            }),
            _ => None,
        },
    )
}

pub(crate) fn extract_logic_bool(value: &TypeIoObject) -> Option<LogicValueExtraction<bool>> {
    extract_logic_value(
        value,
        |object| match object {
            TypeIoObject::Bool(flag) => Some(LogicValueExtraction {
                value: *flag,
                source: "bool",
            }),
            TypeIoObject::ObjectArray(_) => None,
            _ => None,
        },
        |object| matches!(object, TypeIoObject::Bool(_)),
        |object| match object {
            TypeIoObject::Bool(flag) => Some(LogicValueExtraction {
                value: *flag,
                source: "bool_nested",
            }),
            _ => None,
        },
    )
}

pub(crate) fn extract_logic_number(value: &TypeIoObject) -> Option<String> {
    logic_number_value(value).or_else(|| {
        find_first_nested_logic_match(value, |object| logic_number_value(object).is_some())
            .and_then(|matched| logic_number_value(matched.value))
    })
}

pub(crate) fn logic_number_value(value: &TypeIoObject) -> Option<String> {
    match value {
        TypeIoObject::Int(number) => Some(number.to_string()),
        TypeIoObject::Long(number) => Some(number.to_string()),
        TypeIoObject::Float(number) => number.is_finite().then(|| number.to_string()),
        TypeIoObject::Double(number) => number.is_finite().then(|| number.to_string()),
        _ => None,
    }
}

fn extract_logic_value<T>(
    value: &TypeIoObject,
    direct: impl Fn(&TypeIoObject) -> Option<LogicValueExtraction<T>>,
    nested_match: impl Fn(&TypeIoObject) -> bool,
    nested: impl Fn(&TypeIoObject) -> Option<LogicValueExtraction<T>>,
) -> Option<LogicValueExtraction<T>> {
    direct(value).or_else(|| {
        find_first_nested_logic_match(value, nested_match)
            .and_then(|matched| nested(matched.value))
    })
}

fn find_first_nested_logic_match<'a, P>(
    value: &'a TypeIoObject,
    predicate: P,
) -> Option<mdt_typeio::TypeIoObjectMatch<'a>>
where
    P: Fn(&TypeIoObject) -> bool,
{
    let TypeIoObject::ObjectArray(children) = value else {
        return None;
    };

    let mut queue: VecDeque<mdt_typeio::TypeIoObjectMatch<'a>> = children
        .iter()
        .enumerate()
        .map(|(index, child)| mdt_typeio::TypeIoObjectMatch {
            value: child,
            path: vec![index],
        })
        .collect();

    while let Some(current) = queue.pop_front() {
        if predicate(current.value) {
            return Some(current);
        }
        if let TypeIoObject::ObjectArray(children) = current.value {
            for (index, child) in children.iter().enumerate() {
                let mut path = current.path.clone();
                path.push(index);
                queue.push_back(mdt_typeio::TypeIoObjectMatch { value: child, path });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdt_typeio::pack_point2;

    #[test]
    fn logic_extractors_match_direct_and_nested_payloads() {
        assert_eq!(
            extract_logic_string(&TypeIoObject::String(Some("alpha".to_string()))),
            Some("alpha".to_string())
        );
        assert_eq!(
            extract_logic_string(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::Bool(false),
                TypeIoObject::String(Some("beta".to_string())),
            ])),
            Some("beta".to_string())
        );

        let world_pos = extract_logic_world_pos(&TypeIoObject::ObjectArray(vec![
            TypeIoObject::Bool(false),
            TypeIoObject::Vec2 { x: 7.0, y: 9.0 },
        ]))
        .expect("expected nested world pos");
        assert_eq!(world_pos.value, (7.0, 9.0));
        assert_eq!(world_pos.source, "vec2_nested");

        let build_pos = extract_logic_build_pos(&TypeIoObject::ObjectArray(vec![
            TypeIoObject::Bool(false),
            TypeIoObject::Long(301),
        ]))
        .expect("expected nested build pos");
        assert_eq!(build_pos.value, 301);
        assert_eq!(build_pos.source, "long_nested");

        let unit_id = extract_logic_unit_id(&TypeIoObject::UnitId(17))
            .expect("expected direct unit id");
        assert_eq!(unit_id.value, 17);
        assert_eq!(unit_id.source, "unit_id");

        let team = extract_logic_team(&TypeIoObject::ObjectArray(vec![
            TypeIoObject::Bool(false),
            TypeIoObject::Long(5),
        ]))
        .expect("expected nested team");
        assert_eq!(team.value, 5);
        assert_eq!(team.source, "long_nested");

        let flag = extract_logic_bool(&TypeIoObject::ObjectArray(vec![
            TypeIoObject::Bool(true),
        ]))
        .expect("expected nested bool");
        assert!(flag.value);
        assert_eq!(flag.source, "bool_nested");

        assert_eq!(logic_number_value(&TypeIoObject::Double(12.5)), Some("12.5".to_string()));
        assert_eq!(
            extract_logic_number(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::Bool(false),
                TypeIoObject::Float(12.5),
            ])),
            Some("12.5".to_string())
        );

        let _ = pack_point2(7, 9);
    }

    #[test]
    fn logic_extractors_prefer_shallower_nested_matches_over_deeper_branch_matches() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![TypeIoObject::Vec2 {
                x: 1.0,
                y: 2.0,
            }])]),
            TypeIoObject::Vec2 { x: 3.0, y: 4.0 },
        ]);

        let world_pos = extract_logic_world_pos(&value).expect("expected shallow world pos");
        assert_eq!(world_pos.value, (3.0, 4.0));
        assert_eq!(world_pos.source, "vec2_nested");

        let number = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![TypeIoObject::Float(
                1.0,
            )])]),
            TypeIoObject::Float(2.0),
        ]);

        assert_eq!(extract_logic_number(&number), Some("2".to_string()));
    }

    #[test]
    fn logic_number_value_rejects_non_finite_float_payloads() {
        assert_eq!(logic_number_value(&TypeIoObject::Float(f32::INFINITY)), None);
        assert_eq!(logic_number_value(&TypeIoObject::Double(f64::NAN)), None);
        assert_eq!(
            extract_logic_number(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::Bool(false),
                TypeIoObject::Float(f32::INFINITY),
            ])),
            None
        );
    }
}
