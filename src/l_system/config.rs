use std::{cell::RefCell, collections::HashMap};

use is_none_or::IsNoneOr;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

use super::RenderConfig;

#[derive(Debug, Clone)]
pub enum Value {
    Range { min: f32, max: f32 },
    Exact(f32),
}

#[derive(Debug, Clone)]
pub enum Values {
    Multiple(Vec<Value>),
    Exact(Value),
    Default,
}

impl Values {
    pub fn new(chars: &mut std::iter::Peekable<std::str::Chars>) -> Self {
        if let Some('(') = chars.peek() {
            let tmp_chars = chars.clone().skip(1);
            let mut j = 1;
            for sym in tmp_chars {
                if sym == ')' {
                    let string = String::from_iter(
                        chars
                            .clone()
                            .take(j)
                            .filter(|&e| e.is_numeric() || matches!(e, '~' | ',' | '.' | '-')),
                    );
                    let values = string
                        .split(',')
                        .flat_map(|string| {
                            let nums = string
                                .split('~')
                                .flat_map(|e| e.parse::<f32>())
                                .collect::<Vec<f32>>();
                            if nums.is_empty() {
                                return None;
                            }
                            if nums.len() == 1 {
                                Some(Value::Exact(nums[0]))
                            } else {
                                Some(Value::Range {
                                    min: nums[0],
                                    max: nums[nums.len() - 1],
                                })
                            }
                        })
                        .collect::<Vec<_>>();

                    chars.nth(j);
                    return if values.len() == 1 {
                        Self::Exact(values[0].clone())
                    } else {
                        Self::Multiple(values)
                    };
                }

                if !sym.is_numeric() && !matches!(sym, '~' | ' ' | ',' | '.' | '-') {
                    break;
                }

                j += 1;
            }
        }

        Self::Default
    }

    pub fn get(&self, default: f32, rng: &mut super::LRng) -> f32 {
        let val = match self {
            Values::Multiple(vec) => {
                let i = rng.gen_range(0..vec.len());
                &vec[i]
            }
            Values::Exact(val) => val,
            Values::Default => return default,
        };

        match val {
            Value::Range { min, max } => rng.gen_range(*min..*max),
            Value::Exact(value) => *value,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LSymbol {
    Scope,
    ScopeEnd,
    Rule(char),
    Object { id: char },
    RotateX(Values),
    RotateNegX(Values),
    RotateY(Values),
    RotateNegY(Values),
    RotateZ(Values),
    RotateNegZ(Values),
    Scale(Values),
}

#[derive(Debug)]
pub struct LRule {
    pub result: Vec<LSymbol>,
    pub chance: f32,
    pub min_gen: Option<f32>,
    pub max_gen: Option<f32>,
}

#[derive(Debug)]
pub struct LRuleSet {
    pub chance: f32,
    pub rules: Vec<LRule>,
}

#[derive(Debug)]
pub struct LRuleSets {
    current: usize,
    sets: Vec<LRuleSet>,
}

#[derive(Default, Debug)]
pub struct LSystemBuildConfig {
    pub iterations: u32,
    pub initial: Vec<LSymbol>,
    pub rule_sets: HashMap<char, LRuleSets>,
}

pub struct LConfig {
    pub rendering: RenderConfig,
    pub rules: LSystemBuildConfig,
    pub interpolation: f32,
    pub(crate) rng: RefCell<ChaCha20Rng>,
    pub seed: u64,
}

impl Default for LConfig {
    fn default() -> Self {
        Self {
            rendering: Default::default(),
            rules: Default::default(),
            interpolation: Default::default(),
            rng: RefCell::new(rand_chacha::ChaCha20Rng::seed_from_u64(0)),
            seed: 0,
        }
    }
}

impl LConfig {
    pub fn from_json(json: String) -> Result<Self, String> {
        match serde_json::from_str::<json::LConfigJSON>(&json) {
            Ok(json::LConfigJSON { rendering, rules }) => Ok(Self {
                rendering,
                rules: rules.into(),
                ..Default::default()
            }),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn get_rule(&self, id: &char, age: f32) -> Option<&[LSymbol]> {
        self.rules.rule_sets.get(id).and_then(|sets| {
            let rules = &sets.sets[sets.current].rules;
            pick_rule(rules, age, &mut self.rng.borrow_mut())
        })
    }

    pub fn randomize_rule_sets(&mut self, n: Option<u32>) {
        let rng = self.rng.get_mut();
        if let Some(n) = n {
            let mut indices = self.rules.rule_sets.keys().copied().collect::<Vec<_>>();
            for _ in 0..n.min(indices.len() as u32) {
                let i = rng.gen_range(0..indices.len());
                let key = indices.remove(i);
                let set = self.rules.rule_sets.get_mut(&key).unwrap();
                set.current = rng.gen_range(0..set.sets.len());
            }
        } else {
            self.rules
                .rule_sets
                .iter_mut()
                .for_each(|(_, set)| set.current = rng.gen_range(0..set.sets.len()));
        }
    }

    pub fn reseed(&mut self, seed: u64) {
        *self.rng.borrow_mut() = rand_chacha::ChaCha20Rng::seed_from_u64(seed);
        self.seed = seed;
    }
}

fn pick_rule<'rules>(
    rules: &'rules [LRule],
    age: f32,
    rng: &mut super::LRng,
) -> Option<&'rules [LSymbol]> {
    let filtered = rules.iter().filter(|rule| {
        rule.min_gen.is_none_or(|v| age >= v) && rule.max_gen.is_none_or(|v| age < v)
    });
    let max_chance = filtered.clone().fold(0.0, |acc, rule| acc + rule.chance);
    if max_chance <= 0.0 {
        return None;
    }
    let n = rng.gen_range(0.0..max_chance);
    let mut t = 0.0;
    for rule in filtered {
        t += rule.chance;
        if t > n {
            return Some(&rule.result);
        }
    }
    None
}

mod json {
    use std::collections::HashMap;

    use serde::Deserialize;

    use crate::l_system::RenderConfig;

    use super::{LRule, LRuleSet, LRuleSets, LSymbol, LSystemBuildConfig, Values};

    #[derive(Deserialize, Debug, Clone)]
    pub(crate) struct RuleJSON {
        pub(crate) result: String,
        #[serde(default)]
        pub(crate) chance: Option<f32>,
        #[serde(default)]
        pub(crate) min_gen: Option<f32>,
        #[serde(default)]
        pub(crate) max_gen: Option<f32>,
    }

    #[derive(Deserialize, Debug, Clone)]
    pub(crate) struct RuleSetJSON {
        pub(crate) rules: Vec<RuleJSON>,
        #[serde(default)]
        pub(crate) chance: Option<f32>,
    }

    #[derive(Deserialize, Clone)]
    pub(crate) struct LSystemBuildConfigJSON {
        #[serde(default)]
        pub(crate) iterations: u32,
        pub(crate) initial: String,
        pub(crate) rules: HashMap<char, Vec<RuleSetJSON>>,
    }

    #[derive(Deserialize)]
    pub(crate) struct LConfigJSON {
        pub(crate) rendering: RenderConfig,
        pub(crate) rules: LSystemBuildConfigJSON,
    }

    impl From<LSystemBuildConfigJSON> for LSystemBuildConfig {
        fn from(val: LSystemBuildConfigJSON) -> Self {
            let LSystemBuildConfigJSON {
                iterations,
                initial,
                rules,
            } = val;

            let initial = string_to_symbols(initial);
            let rule_sets = rules
                .into_iter()
                .map(|(key, rule_sets)| {
                    let (remaining_chance, remaining_to_fill) =
                        rule_sets.iter().fold((1.0, 0), |mut acc, rule| {
                            if let Some(chance) = rule.chance {
                                acc.0 -= chance;
                            } else {
                                acc.1 += 1;
                            }

                            acc
                        });
                    let divided_chance = remaining_chance / remaining_to_fill as f32;

                    let rule_sets = rule_sets
                        .into_iter()
                        .map(|RuleSetJSON { rules, chance }| {
                            let rules = {
                                let (remaining_chance, remaining_to_fill) =
                                    rules.iter().fold((1.0, 0), |mut acc, rule| {
                                        if let Some(chance) = rule.chance {
                                            acc.0 -= chance;
                                        } else {
                                            acc.1 += 1;
                                        }

                                        acc
                                    });

                                let divided_chance = remaining_chance / remaining_to_fill as f32;
                                rules
                                    .into_iter()
                                    .map(
                                        |RuleJSON {
                                             result,
                                             chance,
                                             min_gen,
                                             max_gen,
                                         }| LRule {
                                            result: string_to_symbols(result),
                                            chance: chance.unwrap_or(divided_chance),
                                            min_gen,
                                            max_gen,
                                        },
                                    )
                                    .collect()
                            };
                            LRuleSet {
                                chance: chance.unwrap_or(divided_chance),
                                rules,
                            }
                        })
                        .collect();
                    let sets = LRuleSets {
                        current: 0,
                        sets: rule_sets,
                    };
                    (key, sets)
                })
                .collect::<HashMap<char, LRuleSets>>();

            LSystemBuildConfig {
                iterations,
                initial,
                rule_sets,
            }
        }
    }

    fn string_to_symbols(string: String) -> Vec<LSymbol> {
        let mut symbols = Vec::with_capacity(string.capacity());
        let mut chars = string.chars().peekable();

        while let Some(symbol) = chars.next() {
            match symbol {
                '[' => symbols.push(LSymbol::Scope),
                ']' => symbols.push(LSymbol::ScopeEnd),
                '+' | '-' | '&' | '^' | '\\' | '/' | '>' | '<' | '|' => {
                    let values = Values::new(&mut chars);
                    let symbol = match symbol {
                        '+' => LSymbol::RotateY(values),
                        '-' => LSymbol::RotateNegY(values),
                        '&' => LSymbol::RotateX(values),
                        '^' => LSymbol::RotateNegX(values),
                        '\\' | '<' => LSymbol::RotateZ(values),
                        '/' | '>' => LSymbol::RotateNegZ(values),
                        '|' => LSymbol::Scale(values),
                        _ => continue,
                    };

                    symbols.push(symbol);
                }
                symbol if symbol.is_ascii() && symbol.is_lowercase() => {
                    symbols.push(LSymbol::Object { id: symbol });
                }
                symbol if symbol.is_ascii() && symbol.is_uppercase() => {
                    symbols.push(LSymbol::Rule(symbol));
                }
                _ => {}
            }
        }

        symbols
    }
}
