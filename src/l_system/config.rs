use std::collections::HashMap;

use super::RenderConfig;

#[derive(Debug, Clone)]
pub enum Value {
    Range { min: f32, max: f32 },
    Exact(f32),
    Default,
}

#[derive(Debug, Clone)]
pub enum LSymbol {
    Scope,
    ScopeEnd,
    Rule(char),
    Object { id: char, age: u32 },
    RotateX(Value),
    RotateNegX(Value),
    RotateY(Value),
    RotateNegY(Value),
    RotateZ(Value),
    RotateNegZ(Value),
    Scale(Value),
}

#[derive(Debug)]
pub struct LRule {
    pub result: Vec<LSymbol>,
    pub chance: f32,
    pub min_gen: Option<u32>,
    pub max_gen: Option<u32>,
}

#[derive(Default, Debug)]
pub struct LSystemBuildConfig {
    pub iterations: u32,
    pub initial: Vec<LSymbol>,
    pub rules: HashMap<char, Vec<LRule>>,
}

pub struct LConfig {
    pub rendering: RenderConfig,
    pub rules: LSystemBuildConfig,
}
mod json {
    use std::collections::HashMap;

    use serde::Deserialize;

    use crate::l_system::RenderConfig;

    use super::{LRule, LSymbol, LSystemBuildConfig, Value};

    #[derive(Deserialize, Debug, Clone)]
    pub(crate) struct RuleJSON {
        pub(crate) result: String,
        #[serde(default = "default_chance")]
        pub(crate) chance: f32,
        #[serde(default)]
        pub(crate) min_gen: Option<u32>,
        #[serde(default)]
        pub(crate) max_gen: Option<u32>,
    }

    fn default_chance() -> f32 {
        1.0
    }

    #[derive(Deserialize, Clone)]
    pub(crate) struct LSystemBuildConfigJSON {
        pub(crate) iterations: u32,
        pub(crate) initial: String,
        pub(crate) rules: HashMap<char, Vec<RuleJSON>>,
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
            let rules = rules
                .into_iter()
                .map(|(key, rules)| {
                    let rules = rules
                        .into_iter()
                        .map(
                            |RuleJSON {
                                 result,
                                 chance,
                                 min_gen,
                                 max_gen,
                             }| LRule {
                                result: string_to_symbols(result),
                                chance,
                                min_gen,
                                max_gen,
                            },
                        )
                        .collect();
                    (key, rules)
                })
                .collect::<HashMap<char, Vec<LRule>>>();

            LSystemBuildConfig {
                iterations,
                initial,
                rules,
            }
        }
    }

    fn string_to_symbols(string: String) -> Vec<LSymbol> {
        let mut symbols = Vec::with_capacity(string.capacity());
        let mut chars = string.chars().peekable();

        let parse_value =
            |chars: &mut std::iter::Peekable<std::str::Chars>| {
                if let Some('(') = chars.peek() {
                    let tmp_chars = chars.clone().skip(1);
                    let mut j = 1;
                    for sym in tmp_chars {
                        if sym == ')' {
                            let string =
                                String::from_iter(chars.clone().take(j).filter(|&e| {
                                    e.is_numeric() || e == '~' || e == '-' || e == '.'
                                }));
                            let nums = string
                                .split('~')
                                .flat_map(|e| e.parse::<f32>())
                                .collect::<Vec<f32>>();
                            if nums.is_empty() {
                                break;
                            }
                            chars.nth(j);
                            return if nums.len() == 1 {
                                Value::Exact(nums[0])
                            } else {
                                Value::Range {
                                    min: nums[0],
                                    max: nums[nums.len() - 1],
                                }
                            };
                        }

                        if !sym.is_numeric() && sym != '~' && sym != '.' && sym != '-' {
                            break;
                        }

                        j += 1;
                    }
                }

                Value::Default
            };

        while let Some(symbol) = chars.next() {
            match symbol {
                '[' => symbols.push(LSymbol::Scope),
                ']' => symbols.push(LSymbol::ScopeEnd),
                '+' | '-' | '&' | '^' | '\\' | '/' | '>' | '<' | '|' => {
                    let value = parse_value(&mut chars);
                    let symbol = match symbol {
                        '+' => LSymbol::RotateY(value),
                        '-' => LSymbol::RotateNegY(value),
                        '&' => LSymbol::RotateX(value),
                        '^' => LSymbol::RotateNegX(value),
                        '\\' | '<' => LSymbol::RotateZ(value),
                        '/' | '>' => LSymbol::RotateNegZ(value),
                        '|' => LSymbol::Scale(value),
                        _ => continue,
                    };

                    symbols.push(symbol);
                }
                symbol if symbol.is_ascii() && symbol.is_lowercase() => {
                    symbols.push(LSymbol::Object { id: symbol, age: 0 });
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

impl LConfig {
    pub fn from_json(json: String) -> Self {
        let json::LConfigJSON { rendering, rules } =
            serde_json::from_str::<json::LConfigJSON>(&json).unwrap();

        let rules = rules.into();
        Self { rendering, rules }
    }
}
