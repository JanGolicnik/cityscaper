use std::collections::HashMap;

use jandering_engine::types::{Qua, Vec3};
use prev_iter::PrevPeekable;
use rand::{rngs::ThreadRng, Rng};
use serde::Deserialize;

use crate::l_system::config::LSymbol;

use super::LSystem;

#[derive(serde::Deserialize, Clone)]
pub enum Shape {
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
pub struct Color {
    age: u32,
    color: [f32; 3],
}

#[derive(Deserialize, Clone)]
pub struct RenderConfig {
    pub default_angle_change: f32,
    pub shapes: HashMap<char, Shape>,
    pub colors: Vec<Color>,
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
pub(super) struct State {
    pub(super) rotation: Qua,
    pub(super) position: Vec3,
    pub(super) scale: f32,
}

impl LSystem {
    pub fn build(&self, render_config: &RenderConfig, rng: &mut ThreadRng) -> Vec<RenderShape> {
        let mut shapes = Vec::new();

        let symbol_to_axis = |symbol: &LSymbol| match &symbol {
            LSymbol::RotateY(_) => Vec3::Y,
            LSymbol::RotateNegY(_) => -Vec3::Y,
            LSymbol::RotateX(_) => Vec3::X,
            LSymbol::RotateNegX(_) => -Vec3::X,
            LSymbol::RotateZ(_) => Vec3::Z,
            LSymbol::RotateNegZ(_) => -Vec3::Z,
            _ => Vec3::ZERO,
        };

        let mut states = vec![State {
            scale: 1.0,
            ..Default::default()
        }];

        for symbol in self.symbols.iter() {
            match symbol {
                LSymbol::Scope => states.push(states.last().unwrap().clone()),
                LSymbol::ScopeEnd => {
                    if states.len() > 1 {
                        states.pop();
                    } else {
                        states[0] = State::default()
                    }
                }
                LSymbol::Object { id, age } => {
                    if let Some(shape) =
                        Self::get_shape(id, age, render_config, states.last_mut().unwrap())
                    {
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
                        super::config::Value::Range { min, max } => rng.gen_range(*min..*max),
                        super::config::Value::Exact(value) => *value,
                        super::config::Value::Default => render_config.default_angle_change,
                    };
                    states.last_mut().unwrap().rotation *=
                        Qua::from_axis_angle(symbol_to_axis(symbol), angle_change.to_radians());
                }
                LSymbol::Scale(value) => {
                    let scale = match value {
                        super::config::Value::Range { min, max } => rng.gen_range(*min..*max),
                        super::config::Value::Exact(value) => *value,
                        super::config::Value::Default => continue,
                    };
                    states.last_mut().unwrap().scale *= scale;
                }
                _ => {}
            }
        }

        shapes
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

    pub(super) fn get_shape(
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
                    let color: Vec3 = Self::get_color(age, render_config);
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
}
