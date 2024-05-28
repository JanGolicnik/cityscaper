use jandering_engine::types::{Qua, Vec3};
use rand::{rngs::ThreadRng, Rng};

use super::{
    builder::{RenderShape, State},
    config::{LConfig, LSymbol},
    pick_rule, LSystem,
};

pub fn build_lsystem(config: &LConfig, rng: &mut ThreadRng) -> Vec<RenderShape> {
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
                if let Some(shape) = LSystem::get_shape(
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
                    super::config::Value::Range { min, max } => rng.gen_range(*min..*max),
                    super::config::Value::Exact(value) => *value,
                    super::config::Value::Default => config.rendering.default_angle_change,
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
            LSymbol::Rule(id) => {
                if iteration == config.rules.iterations {
                    continue;
                }

                if let Some(rules) = config.rules.rules.get(&id) {
                    if let Some(rule) = pick_rule(rules, rng, iteration) {
                        build_symbols(states, shapes, rule, config, rng, iteration + 1);
                    }
                }
            }
        }
    }
}
