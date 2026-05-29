use anyhow::anyhow;
use croner::Cron;
use indexmap::IndexMap;
use log::info;
use rgb::RGB8;
use std::str::FromStr;

use crate::{problem::ProblemId, tz::TimeZone};
pub use feature_config_derive::FeatureConfig;

pub fn parse_rgb8(hex_str: &str) -> anyhow::Result<RGB8> {
    let hex_str = hex_str.trim_start_matches('#');
    if hex_str.len() != 6 {
        anyhow::bail!("Invalid color format: {}", hex_str);
    }

    let r = u8::from_str_radix(&hex_str[0..2], 16)?;
    let g = u8::from_str_radix(&hex_str[2..4], 16)?;
    let b = u8::from_str_radix(&hex_str[4..6], 16)?;

    Ok(RGB8 { r, g, b })
}

pub fn format_rgb8(color: &RGB8) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

pub fn format_opt_rgb8(color: &Option<RGB8>) -> String {
    if let Some(color) = color {
        format_rgb8(color)
    } else {
        "".to_string()
    }
}

pub trait FeatureConfig: Sized {
    fn from_config_spec(spec: &ConfigSpec) -> anyhow::Result<Self>;
    fn to_config_spec() -> anyhow::Result<ConfigSpec>;
}

#[derive(Clone, PartialEq)]
pub enum TypedValue {
    String(usize, Option<String>),
    Int32(Option<i32>),
    Int64(Option<i64>),
    Bool(Option<bool>),
    TimeZone(Option<TimeZone>),
    Cron(Option<Cron>),
    Color(Option<RGB8>),
}

impl std::fmt::Debug for TypedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedValue::Cron(Some(c)) => {
                write!(f, "Cron({})", c)
            }
            TypedValue::Cron(None) => {
                write!(f, "Cron(None)")
            }
            TypedValue::String(len, val) => write!(f, "String({}, {:?})", len, val),
            TypedValue::Int32(val) => write!(f, "Int32({:?})", val),
            TypedValue::Int64(val) => write!(f, "Int64({:?})", val),
            TypedValue::Bool(val) => write!(f, "Bool({:?})", val),
            TypedValue::TimeZone(val) => write!(f, "TimeZone({:?})", val),
            TypedValue::Color(val) => write!(f, "Color({:?})", val),
        }
    }
}

impl TypedValue {
    pub fn is_none(&self) -> bool {
        match self {
            TypedValue::String(_len, val) => val.is_none(),
            TypedValue::Int32(val) => val.is_none(),
            TypedValue::Int64(val) => val.is_none(),
            TypedValue::Bool(_) => false, // Bool is never None, it defaults to false
            TypedValue::TimeZone(_) => false, // TimeZone is never None, it defaults to UTC
            TypedValue::Cron(val) => val.is_none(),
            TypedValue::Color(val) => val.is_none(),
        }
    }

    pub fn to_heapless<const N: usize>(&self) -> anyhow::Result<heapless::String<N>> {
        if let TypedValue::String(_len, Some(val)) = self {
            Ok(heapless::String::<N>::try_from(val.as_str())?)
        } else {
            Ok(heapless::String::<N>::try_from(self.to_string().as_str())?)
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            TypedValue::String(_len, Some(val)) => val.clone(),
            TypedValue::Int32(Some(val)) => val.to_string(),
            TypedValue::Int64(Some(val)) => val.to_string(),
            TypedValue::Bool(Some(val)) => val.to_string(),
            TypedValue::TimeZone(Some(tz)) => tz.to_str().to_string(),
            TypedValue::Cron(Some(cron)) => cron.pattern.to_string(),
            TypedValue::Color(Some(color)) => format_rgb8(color),
            _ => "".to_string(),
        }
    }

    pub fn to_none(&self) -> Self {
        match self {
            TypedValue::String(len, _) => TypedValue::String(*len, None),
            TypedValue::Int32(_) => TypedValue::Int32(None),
            TypedValue::Int64(_) => TypedValue::Int64(None),
            TypedValue::Bool(_) => TypedValue::Bool(None),
            TypedValue::TimeZone(_) => TypedValue::TimeZone(None),
            TypedValue::Cron(_) => TypedValue::Cron(None),
            TypedValue::Color(_) => TypedValue::Color(None),
        }
    }

    pub fn from_str(&self, str_val: &str) -> anyhow::Result<TypedValue> {
        Ok(match self {
            TypedValue::String(len, _) => {
                if str_val.len() > *len as usize {
                    anyhow::bail!("String value too long: max length is {}", len);
                } else {
                    TypedValue::String(*len, Some(str_val.to_string()))
                }
            }
            TypedValue::Int32(_) => TypedValue::Int32(Some(str_val.parse::<i32>()?)),
            TypedValue::Int64(_) => TypedValue::Int64(Some(str_val.parse::<i64>()?)),
            TypedValue::Bool(_) => TypedValue::Bool(Some(str_val.parse::<bool>()?)),
            TypedValue::TimeZone(_) => TypedValue::TimeZone(TimeZone::from_str(str_val)),
            TypedValue::Cron(_) => TypedValue::Cron(Some(Cron::from_str(str_val)?)),
            TypedValue::Color(_) => TypedValue::Color(Some(parse_rgb8(str_val)?)),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: EnabledState,
    pub spec: ConfigSpec,
}

impl Config {
    // should be called get_required_as_string
    pub fn get_valid(&self, key: &str) -> anyhow::Result<String> {
        if let Some(value) = self.spec.map.get(key) {
            Ok(value.value.to_string())
        } else {
            Err(anyhow!("Config value {} is missing", key))
        }
    }

    pub fn get_required_as_heapless<const N: usize>(
        &self,
        key: &str,
    ) -> anyhow::Result<heapless::String<N>> {
        if let Some(value) = self.spec.map.get(key) {
            Ok(value.value.to_heapless::<N>()?)
        } else {
            Err(anyhow!("Config value {} is missing", key))
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigSpecValue {
    pub value: TypedValue,
    pub required: bool,
    pub problem_id: ProblemId,
}

impl ConfigSpecValue {
    pub fn new(value: TypedValue, required: bool) -> Self {
        ConfigSpecValue {
            value,
            required,
            problem_id: None,
        }
    }
}

pub struct ConfigSpecBuilder {
    map: IndexMap<String, ConfigSpecValue>,
}

impl ConfigSpecBuilder {
    fn new() -> Self {
        Self {
            map: IndexMap::new(),
        }
    }

    pub fn with(mut self, name: String, value: ConfigSpecValue) -> anyhow::Result<Self> {
        self.insert(name, value)?;
        Ok(self)
    }

    pub fn insert(&mut self, name: String, value: ConfigSpecValue) -> anyhow::Result<()> {
        if self.map.contains_key(&name) {
            anyhow::bail!("Duplicate config name: {}", name);
        }

        if name.len() > 15 {
            anyhow::bail!("Config name \"{}\" is too long: max length is 15", name);
        }

        if name.starts_with("_") {
            anyhow::bail!("Config name \"{}\" is invalid: cannot start with _", name);
        }

        self.map.insert(name, value);
        Ok(())
    }

    pub fn build(mut self) -> ConfigSpec {
        self.map.shrink_to_fit();

        ConfigSpec { map: self.map }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigSpec {
    pub map: IndexMap<String, ConfigSpecValue>,
}

impl ConfigSpec {
    pub fn builder() -> ConfigSpecBuilder {
        ConfigSpecBuilder::new()
    }

    pub fn is_valid(&self) -> bool {
        info!("is_valid():");
        for (_name, config_value) in &self.map {
            info!("is_valid(): {}", _name);
            if config_value.required && config_value.value.is_none() {
                info!("is_valid(): {} IS NOT VALID", _name);
                return false;
            }
        }
        info!("is_valid(): OK");
        true
    }

    pub fn get_valid(&self, key: &str) -> anyhow::Result<String> {
        if let Some(value) = self.map.get(key) {
            Ok(value.value.to_string())
        } else {
            Err(anyhow!("Config value {} is missing", key))
        }
    }
}

pub trait ConfigStoreFactory {
    fn create(&self, name: String, internal: bool) -> anyhow::Result<Box<dyn ConfigStore>>;
}

pub trait ConfigStore: Sync + Send {
    fn erase_all(&self) -> anyhow::Result<()>;
    fn load(&self, name: &str, config_value: &mut ConfigSpecValue);
    fn save(
        &self,
        name: &str,
        config_value: &mut ConfigSpecValue,
        str_value: &str,
    ) -> anyhow::Result<()>;
    fn remove(&self, name: &str, config_value: &mut ConfigSpecValue) -> anyhow::Result<()>;
    fn load_enabled_state(&self) -> anyhow::Result<EnabledState>;
    fn save_enabled_state(&self, enabled_state: EnabledState) -> anyhow::Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnabledState {
    Enabled,
    Disabled,
    Required,
}

impl EnabledState {
    pub fn is_enabled(&self) -> bool {
        matches!(self, EnabledState::Enabled | EnabledState::Required)
    }
}

impl From<bool> for EnabledState {
    fn from(value: bool) -> Self {
        if value {
            EnabledState::Enabled
        } else {
            EnabledState::Disabled
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rgb8_accepts_hash_prefixed_hex_strings() {
        let color = parse_rgb8("#00ff00").expect("should parse green");
        assert_eq!(color, RGB8 { r: 0, g: 255, b: 0 });
    }

    #[test]
    fn parse_rgb8_accepts_hash_prefixed_hex_strings_red() {
        let color = parse_rgb8("#ff0000").expect("should parse red");
        assert_eq!(color, RGB8 { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn parse_rgb8_accepts_plain_hex_strings() {
        let color = parse_rgb8("ff00ff").expect("should parse magenta");
        assert_eq!(
            color,
            RGB8 {
                r: 255,
                g: 0,
                b: 255
            }
        );
    }

    #[test]
    fn parse_rgb8_returns_err_for_invalid_length() {
        assert!(parse_rgb8("#123").is_err());
        assert!(parse_rgb8("12345").is_err());
    }

    #[test]
    fn parse_rgb8_returns_err_for_invalid_hex() {
        assert!(parse_rgb8("#gg0000").is_err());
        assert!(parse_rgb8("zzzzzz").is_err());
    }

    #[test]
    fn format_rgb8_emits_lowercase_hex_with_hash_prefix() {
        let color = RGB8 { r: 1, g: 2, b: 3 };
        assert_eq!(format_rgb8(&color), "#010203");
    }

    #[test]
    fn format_and_parse_rgb8_roundtrip() {
        let original = RGB8 {
            r: 18,
            g: 52,
            b: 86,
        };
        let formatted = format_rgb8(&original);
        let reparsed = parse_rgb8(&formatted).expect("roundtrip parse should succeed");
        assert_eq!(reparsed, original);
    }
}
