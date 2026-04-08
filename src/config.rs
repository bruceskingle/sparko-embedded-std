use indexmap::IndexMap;
use log::info;
use std::{sync::{Arc, Mutex}};

use anyhow::anyhow;

use crate::tz::TimeZone;

#[derive(Clone, Copy)]
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


#[derive(Debug, PartialEq)]
pub enum TypedValue {
    String(usize, Option<String>),
    Int32(Option<i32>),
    Int64(Option<i64>),
    Bool(bool),
    TimeZone(TimeZone),
}

impl TypedValue {
    // pub fn is_type_compatible(&self, other: &TypedValue) -> bool {
    //     match self {
    //         TypedValue::String(len, _value) => {
    //             if let TypedValue::String(other_len, _) = other {
    //                 return len == other_len;
    //             }
    //             false
    //         },
    //         TypedValue::Int32(_) => matches!(other, TypedValue::Int32(_)),
    //         TypedValue::Int64(_) => matches!(other, TypedValue::Int64(_) ),
    //         TypedValue::Bool(_) => matches!(other, TypedValue::Bool(_)),
    //     }
    // }

    pub fn is_none(&self) -> bool {
        match self {
            TypedValue::String(_len, val) => val.is_none(),
            TypedValue::Int32(val) => val.is_none(),
            TypedValue::Int64(val) => val.is_none(),
            TypedValue::Bool(_) => false, // Bool is never None, it defaults to false
            TypedValue::TimeZone(_) => false, // TimeZone is never None, it defaults to a specific timezone
        }
    }
    
    pub fn to_string(&self) -> String {
        match self {
            TypedValue::String(_len, Some(val)) => val.clone(),
            TypedValue::Int32(Some(val)) => val.to_string(),
            TypedValue::Int64(Some(val)) => val.to_string(),
            TypedValue::Bool(val) => val.to_string(),
            TypedValue::TimeZone(tz) => tz.to_str().to_string(),
            _ => "".to_string(),
        }
    }

    pub fn to_none(&self) -> Self {
        match self {
            TypedValue::String(len, val) => TypedValue::String(*len, None),
            TypedValue::Int32(val) => TypedValue::Int32(None),
            TypedValue::Int64(val) => TypedValue::Int64(None),
            TypedValue::Bool(val) => TypedValue::Bool(false),
            TypedValue::TimeZone(_) => TypedValue::TimeZone(TimeZone::Utc),
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
                
            },
            TypedValue::Int32(_) => TypedValue::Int32(Some(str_val.parse::<i32>()?)),
            TypedValue::Int64(_) => TypedValue::Int64(Some(str_val.parse::<i64>()?)),
            TypedValue::Bool(_) => TypedValue::Bool(str_val.parse::<bool>()?),
            TypedValue::TimeZone(_) => {
                if let Some(tz) = TimeZone::from_str(str_val) {
                    TypedValue::TimeZone(tz)
                } else {
                    anyhow::bail!("Invalid timezone value: {}", str_val);
                }
            },
        })
    }
}


#[derive(Debug)]
pub struct ConfigValue {
    pub value: TypedValue,
    pub required: bool,
}


pub struct ConfigBuilder {
    map: IndexMap<String, ConfigValue>,
}

impl ConfigBuilder {
    fn new() -> Self {
        Self { map: IndexMap::new() }
    }

    pub fn with(mut self, name: String, value: ConfigValue) -> anyhow::Result<Self> {
        self.insert(name, value)?;
        Ok(self)
    }

    pub fn insert(&mut self, name: String, value: ConfigValue) -> anyhow::Result<()> {
        if self.map.contains_key(&name) {
            anyhow::bail!("Duplicate config name: {}", name);
        }

        if name.len() > 15{
            anyhow::bail!("Config name \"{}\" is too long: max length is 15", name);
        }

        if name.starts_with("_") {
            anyhow::bail!("Config name \"{}\" is invalid: cannot start with _", name);
        }   
        
        self.map.insert(name, value);
        Ok(())
    }

    pub fn build(mut self) -> Config {
        self.map.shrink_to_fit();

        Config {
            map: self.map,
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub map: IndexMap<String, ConfigValue>,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    pub fn is_valid(&self, config_name: &str) -> bool {
        for (name, config_value) in &self.map {
            if config_value.required && config_value.value.is_none() {
                log::error!("Missing required config value: {} in {}", name, config_name);
                return false;
            }
        }
        true
    }
    
    pub fn get_valid(&self, key: &str) -> anyhow::Result<String> {
        if let Some(value) = self.map.get(key) {
            Ok(value.value.to_string())
        }
        else {
            Err(anyhow!("Config value {} is missing", key))
        }
    }
}