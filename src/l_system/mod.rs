use std::collections::HashMap;

use rand::{rngs::ThreadRng, Rng};

use self::config::{LRule, LSymbol, LSystemBuildConfig};

pub mod builder;
pub mod config;
pub mod test;

#[derive(Debug)]
pub struct LSystem {
    symbols: Vec<LSymbol>,
}

impl LSystem {
    pub fn new(config: &LSystemBuildConfig, rng: &mut ThreadRng) -> Self {
        let LSystemBuildConfig {
            iterations,
            initial,
            rules,
        } = config;

        let mut symbols = initial.clone();
        for i in 0..*iterations {
            symbols = iterate(symbols, rules, rng, i);
        }
        Self { symbols }
    }
}

fn iterate(
    symbols: Vec<LSymbol>,
    rules: &HashMap<char, Vec<LRule>>,
    rng: &mut ThreadRng,
    iteration: u32,
) -> Vec<LSymbol> {
    let mut new_symbols = Vec::with_capacity(symbols.capacity() * 2);

    for symbol in symbols {
        match symbol {
            LSymbol::Rule(id) => {
                if let Some(rules) = rules.get(&id) {
                    if let Some(rule) = pick_rule(rules, rng, iteration) {
                        new_symbols.reserve(rule.len());
                        let mut rule = rule.clone();
                        rule.iter_mut().for_each(|e| {
                            if let LSymbol::Object { age, .. } = e {
                                *age = iteration;
                            }
                        });
                        new_symbols.append(&mut rule);
                    }
                }
            }
            _ => new_symbols.push(symbol),
        }
    }

    new_symbols
}

pub(super) fn pick_rule<'rules>(
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
