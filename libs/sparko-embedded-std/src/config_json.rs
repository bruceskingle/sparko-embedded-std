//! The config as JSON, for an app to show and change it through the device's API, as the /config page does for a
//! browser. It describes each setting as well as giving its value, so that the app can draw a form for any feature
//! without knowing it in advance.
//!
//! A secret's value is never given, only whether it is set, see ConfigSpecValue::secret.

use anyhow::anyhow;
use serde_json::{Map, Value, json};

use crate::config::{ConfigSpecValue, EnabledState, TypedValue};
use crate::config_manager::ConfigManager;
use crate::tz::TimeZone;

fn type_name(value: &TypedValue) -> &'static str {
    match value {
        TypedValue::String(_, _) => "string",
        TypedValue::Int32(_) => "int32",
        TypedValue::Int64(_) => "int64",
        TypedValue::Bool(_) => "bool",
        TypedValue::TimeZone(_) => "timeZone",
        TypedValue::Cron(_) => "cron",
        TypedValue::Color(_) => "color",
    }
}

fn value_json(value: &TypedValue) -> Value {
    match value {
        TypedValue::Int32(Some(value)) => json!(value),
        TypedValue::Int64(Some(value)) => json!(value),
        TypedValue::Bool(Some(value)) => json!(value),
        value if value.is_none() => Value::Null,
        value => json!(value.to_string()),
    }
}

fn setting_json(name: &str, setting: &ConfigSpecValue) -> Value {
    let mut json = Map::new();
    json.insert("name".to_string(), json!(name));
    json.insert("type".to_string(), json!(type_name(&setting.value)));
    json.insert("required".to_string(), json!(setting.required));
    if let TypedValue::String(len, _) = setting.value {
        json.insert("maxLength".to_string(), json!(len));
    }
    if setting.secret {
        json.insert("secret".to_string(), json!(true));
        json.insert("isSet".to_string(), json!(!setting.value.is_none()));
    } else {
        json.insert("value".to_string(), value_json(&setting.value));
    }
    match &setting.value {
        TypedValue::TimeZone(_) => {
            let options: Vec<&str> = TimeZone::iter().map(|tz| tz.to_str()).collect();
            json.insert("options".to_string(), json!(options));
        }
        TypedValue::Cron(Some(cron)) => {
            json.insert("description".to_string(), json!(cron.describe()));
        }
        _ => {}
    }
    Value::Object(json)
}

/// The text form of a value given as JSON, as the /config page's form would give it, None for null or empty, which
/// clears an optional setting.
fn value_text(value: &Value) -> anyhow::Result<Option<String>> {
    Ok(match value {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => anyhow::bail!("must be a string, number, boolean or null"),
    })
}

impl ConfigManager {
    /// Every feature's config, as {"features": [{"name", "enabled", "settings": [...]}]}, where enabled is "required"
    /// for a feature which is always enabled, e.g. core, otherwise true or false.
    pub fn to_json(&self) -> Value {
        let features: Vec<Value> = self
            .features
            .values()
            .map(|feature| {
                let config = feature.inner.lock().unwrap();
                let enabled = match config.enabled {
                    EnabledState::Required => json!("required"),
                    enabled => json!(enabled.is_enabled()),
                };
                let settings: Vec<Value> =
                    config.spec.map.iter().map(|(name, setting)| setting_json(name, setting)).collect();
                json!({ "name": feature.name, "enabled": enabled, "settings": settings })
            })
            .collect();
        json!({ "features": features })
    }

    /// Changes the config, as {"features": {"<feature>": {"enabled": true, "settings": {"<name>": value}}}}, where
    /// only what is given changes, and null clears an optional setting. Everything is checked before anything is
    /// saved, so that a mistake changes nothing. Like the /config page's changes, they take effect when the device
    /// restarts.
    pub fn apply_json(&self, update: &Value) -> anyhow::Result<()> {
        let features = update
            .get("features")
            .and_then(|features| features.as_object())
            .ok_or_else(|| anyhow!("expected {{\"features\": {{...}}}}"))?;

        // Checked first: (feature, enabled, [(setting, text)]).
        let mut changes = Vec::new();
        for (feature_name, feature_update) in features {
            let feature = self
                .features
                .get(feature_name)
                .ok_or_else(|| anyhow!("there is no feature {feature_name}"))?;
            let config = feature.inner.lock().unwrap();
            let enabled = match feature_update.get("enabled") {
                None | Some(Value::Null) => None,
                Some(Value::Bool(enabled)) => {
                    if let EnabledState::Required = config.enabled {
                        if !*enabled {
                            anyhow::bail!("{feature_name} cannot be disabled");
                        }
                        None
                    } else {
                        Some(EnabledState::from(*enabled))
                    }
                }
                Some(_) => anyhow::bail!("{feature_name} enabled must be true or false"),
            };
            let mut settings = Vec::new();
            if let Some(settings_update) = feature_update.get("settings") {
                let settings_update = settings_update
                    .as_object()
                    .ok_or_else(|| anyhow!("{feature_name} settings must be an object"))?;
                for (name, value) in settings_update {
                    let setting = config
                        .spec
                        .map
                        .get(name)
                        .ok_or_else(|| anyhow!("{feature_name} has no setting {name}"))?;
                    let text = value_text(value).map_err(|error| anyhow!("{feature_name} {name} {error}"))?;
                    match &text {
                        None if setting.required => anyhow::bail!("{feature_name} {name} is required"),
                        None => {}
                        Some(text) => {
                            setting
                                .value
                                .from_str(text)
                                .map_err(|error| anyhow!("{feature_name} {name}: {error}"))?;
                        }
                    }
                    settings.push((name.clone(), text));
                }
            }
            changes.push((feature_name.clone(), enabled, settings));
        }

        for (feature_name, enabled, settings) in changes {
            let feature = &self.features[&feature_name];
            let mut config = feature.inner.lock().unwrap();
            if let Some(enabled) = enabled {
                if enabled != config.enabled {
                    feature.config_store.save_enabled_state(enabled)?;
                    config.enabled = enabled;
                }
            }
            for (name, text) in settings {
                let setting = config.spec.map.get_mut(&name).expect("checked above");
                match text {
                    Some(text) => feature.config_store.save(&name, setting, &text)?,
                    None if !setting.value.is_none() => feature.config_store.remove(&name, setting)?,
                    None => {}
                }
            }
        }
        Ok(())
    }
}
