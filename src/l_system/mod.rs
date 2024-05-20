use std::collections::HashMap;

use rand::Rng;
use serde::Deserialize;

pub mod builder;

#[derive(Deserialize, Debug)]
pub struct Rule {
    result: String,
    chance: f32,
}

#[derive(Deserialize)]
pub struct LSystemConfig {
    iterations: u32,
    initial: String,
    rules: HashMap<char, Vec<Rule>>,
}

impl Default for LSystemConfig {
    fn default() -> Self {
        Self {
            iterations: 0,
            initial: "".to_string(),
            rules: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct LSystem {
    symbols: Vec<char>,
}

impl LSystem {
    pub fn new(config: LSystemConfig) -> Self {
        let LSystemConfig {
            iterations,
            initial,
            rules,
        } = config;
        let mut rng = rand::thread_rng();

        let mut pick_rule = |rules: &[Rule]| {
            let n = rng.gen::<f32>();
            let mut t = 0.0;
            for rule in rules.iter() {
                t += rule.chance;
                if t > n {
                    return Some(rule.result.clone());
                }
            }
            None
        };

        let mut symbols = initial.chars().collect::<Vec<_>>();
        (0..iterations).for_each(|_| {
            let mut new_symbols = Vec::new();
            for symbol in symbols.iter() {
                if let Some(rules) = rules.get(symbol) {
                    let mut new_chars = pick_rule(rules).unwrap().chars().collect::<Vec<_>>();
                    new_symbols.append(&mut new_chars);
                } else {
                    new_symbols.push(*symbol);
                }
            }
            symbols = std::mem::take(&mut new_symbols);
        });

        Self { symbols }
    }
}
