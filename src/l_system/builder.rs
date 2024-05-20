use std::collections::HashMap;

use jandering_engine::types::{Qua, Vec3, DEG_TO_RAD};
use rand::Rng;
use serde::Deserialize;

use super::LSystem;

#[derive(serde::Deserialize, Clone)]
pub enum Shape {
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

#[derive(Deserialize)]
pub struct RenderConfig {
    default_angle_change: f32,
    shapes: HashMap<char, Shape>,
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

impl LSystem {
    pub fn build(&self, render_config: &RenderConfig) -> Vec<RenderShape> {
        let mut shapes = Vec::new();

        let mut rng = rand::thread_rng();

        #[derive(Clone, Default)]
        struct State {
            rotation: Qua,
            position: Vec3,
        }

        let symbol_to_axis = |symbol: &char| match symbol {
            '+' => Vec3::Y,
            '-' => -Vec3::Y,
            '&' => Vec3::X,
            '^' => -Vec3::X,
            '\\' | '<' => Vec3::Z,
            '/' | '>' => -Vec3::Z,
            _ => Vec3::ZERO,
        };

        let mut states = vec![State::default()];

        let mut i = 0;
        while i < self.symbols.len() {
            let symbol = &self.symbols[i];
            match symbol {
                '[' => states.push(states.last().unwrap().clone()),
                ']' => {
                    if states.len() > 1 {
                        states.pop();
                    } else {
                        states[0] = State::default()
                    }
                }
                '+' | '-' | '&' | '^' | '\\' | '/' | '>' | '<' => {
                    let mut angle_change = render_config.default_angle_change;
                    if let Some('(') = self.symbols.get(i + 1) {
                        let mut j = 2;
                        while let Some(sym) = self.symbols.get(i + j) {
                            if *sym == ')' {
                                let string = String::from_iter(
                                    self.symbols[i..i + j]
                                        .iter()
                                        .filter(|e| **e == '-' || **e == '.' || e.is_numeric()),
                                );
                                let nums = string
                                    .split('-')
                                    .flat_map(|e| e.parse::<f32>())
                                    .collect::<Vec<f32>>();
                                if nums.is_empty() {
                                    break;
                                }

                                let t = rng.gen_range(0.0..nums.len() as f32 - 1.0);
                                let lower = nums[t.floor() as usize];
                                let upper = nums[t.ceil() as usize];
                                let t = t.fract();
                                angle_change = lower * (1.0 - t) + upper * t;
                            }
                            if !sym.is_numeric() && *sym != '.' && *sym != '-' {
                                break;
                            }
                            j += 1;
                        }
                    }
                    states.last_mut().unwrap().rotation *=
                        Qua::from_axis_angle(symbol_to_axis(symbol), angle_change * DEG_TO_RAD);
                }
                '|' => {
                    states.last_mut().unwrap().rotation *=
                        Qua::from_rotation_y(std::f32::consts::PI);
                }
                _ => {
                    if let Some(shape) = render_config.shapes.get(symbol) {
                        match shape {
                            Shape::Line {
                                width,
                                length,
                                color,
                            } => {
                                let state = states.last_mut().unwrap();
                                let end = state.position
                                    + state.rotation.mul_vec3(Vec3::new(0.0, *length, 0.0));

                                shapes.push(RenderShape::Line {
                                    start: state.position,
                                    end,
                                    width: *width,
                                    color: Vec3::from(*color),
                                });

                                state.position = end;
                            }
                            Shape::Circle { size, color } => {
                                let state = states.last_mut().unwrap();

                                shapes.push(RenderShape::Circle {
                                    size: *size,
                                    pos: state.position,
                                    color: Vec3::from(*color),
                                });
                            }
                        }
                    }
                }
            }

            i += 1;
        }

        shapes
    }
}
