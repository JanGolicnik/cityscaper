use std::collections::HashMap;

use jandering_engine::types::{Qua, Vec3};
use prev_iter::PrevPeekable;
use rand::{rngs::ThreadRng, Rng};
use serde::Deserialize;

use self::config::{LConfig, LRule, LSymbol, Value};

pub mod config;

#[derive(serde::Deserialize, Clone)]
enum Shape {
    Branch {
        width: f32,
        length: f32,
    },
    Line {
        width: f32,
        length: f32,
        color: [f32; 3],
    },
    Circle {
        size: f32,
        color: [f32; 3],
    },
}

#[derive(Deserialize, Clone)]
struct Color {
    age: u32,
    color: [f32; 3],
}

#[derive(Deserialize, Clone)]
pub struct RenderConfig {
    default_angle_change: f32,
    shapes: HashMap<char, Shape>,
    colors: Vec<Color>,
}

#[derive(Debug)]
pub enum RenderShape {
    Line {
        start: Vec3,
        end: Vec3,
        width: f32,
        color: Vec3,
    },
    Circle {
        size: f32,
        pos: Vec3,
        color: Vec3,
    },
}

#[derive(Clone, Default)]
struct State {
    rotation: Qua,
    position: Vec3,
    scale: f32,
}

pub fn build(config: &LConfig, rng: &mut ThreadRng) -> Vec<RenderShape> {
    let mut states = vec![State {
        scale: 1.0,
        ..Default::default()
    }];

    let mut shapes = Vec::new();

    build_symbols(
        &mut states,
        &mut shapes,
        &config.rules.initial.clone(),
        config,
        rng,
        0,
    );

    shapes
}

fn build_symbols(
    states: &mut Vec<State>,
    shapes: &mut Vec<RenderShape>,
    symbols: &[LSymbol],
    config: &LConfig,
    rng: &mut ThreadRng,
    iteration: u32,
) {
    let symbol_to_axis = |symbol: &LSymbol| match &symbol {
        LSymbol::RotateY(_) => Vec3::Y,
        LSymbol::RotateNegY(_) => -Vec3::Y,
        LSymbol::RotateX(_) => Vec3::X,
        LSymbol::RotateNegX(_) => -Vec3::X,
        LSymbol::RotateZ(_) => Vec3::Z,
        LSymbol::RotateNegZ(_) => -Vec3::Z,
        _ => Vec3::ZERO,
    };

    for symbol in symbols {
        match symbol {
            LSymbol::Scope => states.push(states.last().unwrap().clone()),
            LSymbol::ScopeEnd => {
                if states.len() > 1 {
                    states.pop();
                } else {
                    states[0] = State::default()
                }
            }
            LSymbol::Object { id, .. } => {
                if let Some(shape) = get_shape(
                    id,
                    &iteration,
                    &config.rendering,
                    states.last_mut().unwrap(),
                ) {
                    shapes.push(shape)
                }
            }
            LSymbol::RotateX(value)
            | LSymbol::RotateNegX(value)
            | LSymbol::RotateY(value)
            | LSymbol::RotateNegY(value)
            | LSymbol::RotateZ(value)
            | LSymbol::RotateNegZ(value) => {
                let angle_change = match value {
                    Value::Range { min, max } => rng.gen_range(*min..*max),
                    Value::Exact(value) => *value,
                    Value::Default => config.rendering.default_angle_change,
                };
                states.last_mut().unwrap().rotation *=
                    Qua::from_axis_angle(symbol_to_axis(symbol), angle_change.to_radians());
            }
            LSymbol::Scale(value) => {
                let scale = match value {
                    Value::Range { min, max } => rng.gen_range(*min..*max),
                    Value::Exact(value) => *value,
                    Value::Default => continue,
                };
                states.last_mut().unwrap().scale *= scale;
            }
            LSymbol::Rule(id) => {
                if iteration >= config.rules.iterations {
                    continue;
                }

                if let Some(rules) = config.rules.rules.get(id) {
                    if let Some(rule) = pick_rule(rules, rng, iteration) {
                        build_symbols(states, shapes, rule, config, rng, iteration + 1);
                    }
                }
            }
        }
    }
}

fn get_color(age: &u32, render_config: &RenderConfig) -> Vec3 {
    let mut iter = PrevPeekable::new(render_config.colors.iter());
    if let Some(second) = iter.find(|e| e.age >= *age) {
        if let Some(first) = iter.prev() {
            let dif = second.age - first.age;
            let age = age - first.age;
            let first = Vec3::from(first.color);
            let second = Vec3::from(second.color);
            let t = age as f32 / dif as f32;
            first * (1.0 - t) + second * t
        } else {
            second.color.into()
        }
    } else {
        render_config.colors.first().unwrap().color.into()
    }
}

fn get_shape(
    id: &char,
    age: &u32,
    render_config: &RenderConfig,
    state: &mut State,
) -> Option<RenderShape> {
    if let Some(shape) = render_config.shapes.get(id) {
        let shape = match shape {
            Shape::Line {
                width,
                length,
                color,
            } => {
                let end = state.position
                    + state
                        .rotation
                        .mul_vec3(Vec3::new(0.0, *length * state.scale, 0.0));
                let start = state.position;
                state.position = end;
                RenderShape::Line {
                    start,
                    end,
                    width: *width,
                    color: Vec3::from(*color),
                }
            }
            Shape::Circle { size, color } => RenderShape::Circle {
                size: *size * state.scale,
                pos: state.position,
                color: Vec3::from(*color),
            },
            Shape::Branch { width, length } => {
                let end = state.position
                    + state
                        .rotation
                        .mul_vec3(Vec3::new(0.0, *length * state.scale, 0.0));
                let color: Vec3 = get_color(age, render_config);
                let start = state.position;
                state.position = end;
                RenderShape::Line {
                    start,
                    end,
                    width: *width,
                    color,
                }
            }
        };
        Some(shape)
    } else {
        None
    }
}

fn pick_rule<'rules>(
    rules: &'rules [LRule],
    rng: &mut ThreadRng,
    gen: u32,
) -> Option<&'rules Vec<LSymbol>> {
    let n = rng.gen::<f32>();
    let mut t = 0.0;
    for rule in rules.iter() {
        if rule.min_gen.is_some_and(|v| gen < v) {
            continue;
        }
        if rule.max_gen.is_some_and(|v| gen > v) {
            continue;
        }
        t += rule.chance;
        if t > n {
            return Some(&rule.result);
        }
    }
    None
}
