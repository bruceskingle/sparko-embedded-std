use std::sync::Mutex;

use indexmap::IndexMap;
use log::info;

use crate::{config::{Config, ConfigSpec, ConfigStore, ConfigStoreFactory, EnabledState, TypedValue}, tz::TimeZone};


/// This is the descriptor for a feature which it uses to describe itself. 
#[derive(Debug)]
pub struct FeatureDescriptor {
    pub name: String,
    pub config: ConfigSpec,
}

pub struct InnerFeatureConfig {
    pub enabled: EnabledState,
    pub config: ConfigSpec,
}

impl InnerFeatureConfig {
}

pub struct FeatureConfig {
    pub name: String,
    pub inner: Mutex<InnerFeatureConfig>,
    pub config_store: Box<dyn ConfigStore>,
}

impl FeatureConfig {
    pub fn to_config(&self) -> Config {
        let mut map = IndexMap::new();
        let inner = &self.inner.lock().unwrap();

        for (name, spec) in &inner.config.map {
            map.insert(name.clone(), spec.value.clone());
        }

        Config {
            enabled: inner.enabled,
            map,
        }
    }

    pub fn from_feature(
        feature_descriptor: FeatureDescriptor,
        config_store_factory:
        &Box<dyn ConfigStoreFactory>,
        internal: bool) -> anyhow::Result<Self> {
        let config_store = config_store_factory.create(feature_descriptor.name.clone(), internal)?;

        let enabled = if internal {
            EnabledState::Required
        }
        else {
            config_store.load_enabled_state()?
        };

        let mut config = feature_descriptor.config;
        info!("Loading feature {} config from NVS", &feature_descriptor.name);
        for (name, config_value) in config.map.iter_mut() {
            //config_value.value = 
            config_store.load(name, config_value);
        }
        info!("Finished loading config: {:?}", config);


        let feature_config = Self {
            name: feature_descriptor.name,
            inner: Mutex::new(InnerFeatureConfig { enabled, config }),
            // nvs_namespace,
            // problem_manager: problem_manager.clone(),
            config_store,
        };

        Ok(feature_config)
    }

    pub fn is_valid(&self) -> bool {
        info!("Validating config for feature: {}", self.name);
        let inner = &self.inner.lock().unwrap();
        if inner.enabled.is_enabled() {
            for (name, config_value) in &inner.config.map {
                if config_value.required && config_value.value.is_none() {
                    log::error!("Missing required config value: {} in feature {}", name, self.name);
                    return false;
                }
            }
        }
        else {
            info!("Config for feature {} is not enabled and therefore valid", self.name);
        }
        info!("Config for feature {} is valid", self.name);
        true
    }

    pub fn create_config_page(&self, 
        resp: &mut dyn std::io::Write
        //&mut esp_idf_svc::http::server::Response<&mut EspHttpConnection<'_>>
    ) -> anyhow::Result<()> {
        info!("Creating config page for feature: {}", &self.name);
        let feature_name = &self.name;
        let inner = &self.inner.lock().unwrap();
        if let EnabledState::Required = inner.enabled {
            // Required features are always enabled, so we just show the config page without a checkbox
        }
        else {
            info!("feature.enabled for {}: {}", &self.name, inner.enabled.is_enabled());

            let name = format!("feature_{}", &self.name);
            let checked = if inner.enabled.is_enabled() {
                " checked"
            } else {
                ""
            };

            resp.write(format!(r#"
                        <label for="{name}">{name}</label>
                        <input id="{name}" name="{name}" type="checkbox"{checked}>
                        <h2>{feature_name}</h2>
            "#).as_bytes())?;
        }

        for (name, config_value) in &inner.config.map {
            let value = config_value.value.to_string();
            let input_type_buf: String;
            let input_type = match &config_value.value {
                TypedValue::String(len, _) => {
                    input_type_buf = format!("text\" maxlength=\"{}", len);
                    &input_type_buf
                },
                TypedValue::Int32(_) | TypedValue::Int64(_) => "number",
                TypedValue::Bool(value) => {
                    let checked = if *value {
                        " checked"
                    }
                    else {
                        ""
                    };

                    resp.write(format!(r#"
                                <label for="{name}">{name}</label>
                                <input id="{name}" name="{name}" type="checkbox" value="true" {checked}>
                    "#).as_bytes())?;
                    continue;
                },
                TypedValue::TimeZone(current) => {
                    info!("Config value {} is a TimeZone,", name);

                    resp.write(format!(r#"
                                <!-- Timezone field {name}-->
                        <label for="{name}">{name}</label>
                        <select id="{name}" name="{name}">"#).as_bytes())?;
                    for tz in TimeZone::iter() {
                        let selected_attr = if *tz == *current { " selected" } else { "" };
                        resp.write(format!(r#"<option value="{}"{}>{}</option>"#, tz.to_str(), selected_attr, tz.to_str()).as_bytes())?;
                    }
                    resp.write(format!(r#"</select>"#).as_bytes())?;
                    continue;
                },
                TypedValue::Cron(opt_cron) => {
                    let description = if let Some(cron) = opt_cron {
                        cron.describe()
                    }
                    else {
                        "None".to_string()
                    };

                    resp.write(format!(r#"
                                <!-- Cron field {name}-->
                                <label for="{name}">{name}</label>
                                <input id="{name}" name="{name}" type="text" value="{value}">
                                <input type="text" value="{description}" disabled>
                    "#).as_bytes())?;
                    continue;
                },
            };
            resp.write(format!(r#"
                        <!-- Other field {name}-->
                        <label for="{name}">{name}</label>
                        <input id="{name}" name="{name}" type="{input_type}" autocomplete="off" value="{value}">
            "#).as_bytes())?;
        }

        Ok(())
    }

    pub fn handle_config_form(&self, form: &IndexMap<String, String>) -> anyhow::Result<()> {
        info!("Handling config form for feature: {}", self.name);
        let mut inner = self.inner.lock().unwrap();
        if let EnabledState::Required = inner.enabled {
            // Required features are always enabled, so we just show the config page without a checkbox
        }
        else {
            let name = format!("feature_{}", &self.name);
            let str_val = form.get(&name).map(|s| s.as_str()).unwrap_or("").trim();
            let enabled = EnabledState::from(str_val == "on");
            info!("Feature {} enabled value from form: {} -> enabled={:?}", &self.name, str_val, enabled);
            if enabled != inner.enabled {
                info!("Feature {} enabled value updated", &self.name);
                self.config_store.save_enabled_state(enabled)?;
                inner.enabled = enabled;
            }
        }

        for (name, config_value) in inner.config.map.iter_mut() {
            info!("Processing config value: {}", name);
            let str_val = form.get(name).map(|s| s.as_str()).unwrap_or("").trim();
            if str_val.len() == 0 {
                log::info!("Config value {} is None", name);
                if ! config_value.value.is_none() {
                    log::info!("Setting optional config value {} to None", name);
                    self.config_store.remove(name, config_value)?;
                }
            }
            else {
                self.config_store.save(name, config_value, str_val)?;
            }
        }

        info!("Finished handling form config: {:?}", &inner.config);

        Ok(())
    }
}