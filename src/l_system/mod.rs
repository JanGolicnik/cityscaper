use std::collections::HashMap;

use jandering_engine::types::{Qua, Vec3};
use serde::Deserialize;

use self::config::{LConfig, LSymbol};

pub mod config;

type LRng = rand_chacha::ChaCha20Rng;

#[derive(serde::Deserialize, Clone)]
enum Shape {
    Branch { width: f32, length: f32 },
    Line { width: f32, length: f32 },
    Circle { size: f32 },
}

#[derive(Deserialize, Clone, Default)]
pub struct RenderConfig {
    default_angle_change: f32,
    shapes: HashMap<char, Shape>,

    #[serde(default)]
    pub width_mod: Option<f32>,
}

#[derive(Debug)]
pub enum RenderShape {
    Line {
        start: Vec3,
        end: Vec3,
        width: f32,
        age: f32,
        last_age: f32,
    },
    Circle {
        size: f32,
        pos: Vec3,
        age: f32,
    },
    Scope,
    ScopeEnd,
}

#[derive(Default)]
struct State {
    rotation: Qua,
    position: Vec3,
    scale: f32,
    age: f32,
}

impl State {
    fn clone(&self, age: f32) -> Self {
        let Self {
            rotation,
            position,
            scale,
            ..
        } = *self;

        Self {
            rotation,
            position,
            scale,
            age,
        }
    }
}

pub fn build(config: &LConfig) -> Vec<RenderShape> {
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
        0,
    );

    shapes
}

fn build_symbols(
    states: &mut Vec<State>,
    shapes: &mut Vec<RenderShape>,
    symbols: &[LSymbol],
    config: &LConfig,
    iteration: u32,
) {
    let age = iteration as f32 / config.rules.iterations.saturating_sub(1) as f32;

    let len_mod = {
        let lerp_age = config.interpolation * config.rules.iterations as f32;
        let t = lerp_age / (iteration + 1) as f32;

        // ( 1 - e^(-2x) ) / ( 1 + e^(-2x) )
        let t = {
            let k = std::f32::consts::E.powf(-2.0 * t);
            (1.0 - k) / (1.0 + k)
        };

        // ( 1 / ( 1 + e^(-3.4 * (x * 2.8 - 1.4)) ) )
        // let t = {
        //     let exp = std::f32::consts::E.powf(-3.4 * (t * 2.8 - 1.4));
        //     1.0 / (1.0 + exp)
        // };

        let t = t.powf(((iteration as f32 - lerp_age) / 3.5).max(1.0));

        // t * 0.5 + 0.5
        // let k = 1.0 / (iteration + 1) as f32;
        // t * (1.0 - k) + k
        t
    };

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
            LSymbol::Scope => {
                shapes.push(RenderShape::Scope);
                states.push(states.last().unwrap().clone(age));
            }
            LSymbol::ScopeEnd => {
                shapes.push(RenderShape::ScopeEnd);
                if states.len() > 1 {
                    states.pop();
                } else {
                    states[0] = State::default()
                }
            }
            LSymbol::Object { id } => {
                if let Some(shape) = get_shape(
                    id,
                    age,
                    &config.rendering,
                    states.last_mut().unwrap(),
                    len_mod,
                ) {
                    shapes.push(shape)
                }
            }
            LSymbol::RotateX(values)
            | LSymbol::RotateNegX(values)
            | LSymbol::RotateY(values)
            | LSymbol::RotateNegY(values)
            | LSymbol::RotateZ(values)
            | LSymbol::RotateNegZ(values) => {
                let angle = values.get(
                    config.rendering.default_angle_change,
                    &mut config.rng.borrow_mut(),
                );
                states.last_mut().unwrap().rotation *=
                    Qua::from_axis_angle(symbol_to_axis(symbol), angle.to_radians());
            }
            LSymbol::Scale(values) => {
                states.last_mut().unwrap().scale *= values.get(1.0, &mut config.rng.borrow_mut());
            }
            LSymbol::Rule(id) => {
                if age >= 1.0 || len_mod < 0.001 {
                    continue;
                }

                if let Some(rule) = config.get_rule(id, age) {
                    build_symbols(states, shapes, rule, config, iteration + 1);
                }
            }
        }
    }
}

fn get_shape(
    id: &char,
    age: f32,
    render_config: &RenderConfig,
    state: &mut State,
    len_mod: f32,
) -> Option<RenderShape> {
    if let Some(shape) = render_config.shapes.get(id) {
        let shape = match shape {
            Shape::Line { width, length } => {
                let length = *length * len_mod;
                let end = state.position
                    + state
                        .rotation
                        .mul_vec3(Vec3::new(0.0, length * state.scale, 0.0));
                let start = state.position;
                state.position = end;

                let width = *width * render_config.width_mod.unwrap_or(1.0);

                RenderShape::Line {
                    start,
                    end,
                    width,
                    age,
                    last_age: state.age,
                }
            }
            Shape::Circle { size } => RenderShape::Circle {
                size: *size * state.scale,
                pos: state.position,
                age,
            },
            Shape::Branch { width, length } => {
                let length = *length * len_mod;
                let end = state.position
                    + state
                        .rotation
                        .mul_vec3(Vec3::new(0.0, length * state.scale, 0.0));
                let start = state.position;
                state.position = end;

                let width = *width * render_config.width_mod.unwrap_or(1.0);

                RenderShape::Line {
                    start,
                    end,
                    width,
                    age,
                    last_age: state.age,
                }
            }
        };
        Some(shape)
    } else {
        None
    }
}
