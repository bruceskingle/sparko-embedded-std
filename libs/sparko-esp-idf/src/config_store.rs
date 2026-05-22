use std::str::FromStr;
use std::sync::Arc;

use croner::Cron;
use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use log::info;
use sparko_embedded_std::{config::{ConfigSpecValue, ConfigStore, ConfigStoreFactory, EnabledState, TypedValue}, problem::ProblemManager, tz::{TIMEZONE_LEN, TimeZone}};

use crate::core::CORE_FEATURE_NAME;

const FEATURE_NAMESPACE_NAME: &str = "feature";
const RESERVED_FEATURE_NAMES: [&str; 6] = [
    CORE_FEATURE_NAME,
    FEATURE_NAMESPACE_NAME,
    "wifi",
    "phy",
    "bt_config",
    "nvs.net80211",
];

pub struct EspConfigStoreFactory {
    nvs_partition: EspNvsPartition<NvsDefault>, 
    problem_manager: Arc<ProblemManager>,
}

impl EspConfigStoreFactory {
    pub fn new(
        nvs_partition: EspNvsPartition<NvsDefault>, 
        problem_manager: Arc<ProblemManager>
    ) -> anyhow::Result<Self>
    {
        // let feature_namespace = EspNvs::new(nvs_partition.clone(), FEATURE_NAMESPACE_NAME, true)?;

        Ok(EspConfigStoreFactory{
            nvs_partition,
            // feature_namespace,
            problem_manager,
        })
    }
}

impl ConfigStoreFactory for EspConfigStoreFactory {
    fn create(&self, feature_name: String, internal: bool) -> anyhow::Result<Box<dyn ConfigStore>>{

        if !internal {
            for reserved_name in RESERVED_FEATURE_NAMES.iter() {
                if &feature_name == *reserved_name {
                    return Err(anyhow::anyhow!("Feature name '{}' is reserved and cannot be used", &feature_name));
                }
            }
        }
        let feature_namespace = EspNvs::new(self.nvs_partition.clone(), FEATURE_NAMESPACE_NAME, true)?;
        let nvs_namespace = EspNvs::new(self.nvs_partition.clone(), &feature_name, true)?;

        {
            info!("Iterating over feature {} NVS items for debugging:", &feature_name);
            let mut keys = nvs_namespace.keys(None).unwrap();

            loop {
                match keys.next_key() {
                    Some((key, data_type)) => log::info!("NVS item: {} of type {:?}", key, data_type),
                    None => break,
                }
            }
        }

        Ok(Box::new(EspConfigStore {
            feature_name,
            feature_namespace,
            nvs_namespace,
            problem_manager: self.problem_manager.clone(),
        }))
    }
}


pub struct EspConfigStore {
    feature_name: String,
    feature_namespace: EspNvs<NvsDefault>,
    pub nvs_namespace: EspNvs<NvsDefault>,
    pub problem_manager: Arc<ProblemManager>,
}


impl EspConfigStore {

    fn unwrap_and_log_esp<T>(&self, name: &str, config_value: &mut ConfigSpecValue, result: Result<Option<T>, esp_idf_sys::EspError>) -> Option<T> {
                match result {
                    Ok(opt_str) => {
                        if opt_str.is_none() && config_value.required {
                            config_value.problem_id = self.problem_manager.set(config_value.problem_id, format!("Required value {} is missing", name));
                        }
                        else {
                            self.problem_manager.clear(config_value.problem_id);
                        }

                        opt_str
                    },
                    Err(error) => {
                        let err = format!("NVS error reading {}: {}", name, error);
                        log::error!("{}", &err);
                        config_value.problem_id = self.problem_manager.set(config_value.problem_id, err);
                        None
                    },
                }
    }

    fn unwrap_and_log_cron<T>(&self, name: &str, config_value: &mut ConfigSpecValue, result: Result<T, croner::errors::CronError>) -> Option<T> {
                match result {
                    Ok(opt_str) => {

                        self.problem_manager.clear(config_value.problem_id);
                        Some(opt_str)
                    },
                    Err(error) => {
                        let err = format!("NVS error reading {}: {}", name, error);
                        log::error!("{}", &err);
                        config_value.problem_id = self.problem_manager.set(config_value.problem_id, err);
                        None
                    },
                }
    }
}

impl ConfigStore for EspConfigStore {
    fn erase_all(&self) -> anyhow::Result<()> {
        self.nvs_namespace.erase_all()?;
        Ok(())
    }
    fn load(&self, name: &str, config_value: &mut ConfigSpecValue) {
        info!("Reading config value {} from NVS", name);
        let result = match &config_value.value {
            TypedValue::String(len_ref, _) => {
                let len = *len_ref; // copying to avoid borrow issues
                let mut buf = vec![0u8; (len as usize)+1];

                if let Some(str)= self.unwrap_and_log_esp(name, config_value, self.nvs_namespace.get_str(name, buf.as_mut_slice())) {
                    TypedValue::String(len, Some(str.to_string()))
                } else {
                    TypedValue::String(len, None)
                }
            },
            TypedValue::Int32(_) => TypedValue::Int32(self.unwrap_and_log_esp(name, config_value, self.nvs_namespace.get_i32(name))),
            TypedValue::Int64(_) => TypedValue::Int64(self.unwrap_and_log_esp(name, config_value, self.nvs_namespace.get_i64(name))),
            TypedValue::Bool(_) => {
                let v = if let Some(value) = self.unwrap_and_log_esp(name, config_value, self.nvs_namespace.get_u8(name)) {
                    value != 0
                } else {
                    false
                };
                TypedValue::Bool(v)
            },
            TypedValue::TimeZone(_) => {
                if let Some(str) = self.unwrap_and_log_esp(name, config_value, self.nvs_namespace.get_str(name, &mut [0u8; TIMEZONE_LEN as usize])) {
                    if let Some(tz) = TimeZone::from_str(str) {
                        TypedValue::TimeZone(tz)
                    } else {
                        TypedValue::TimeZone(TimeZone::Utc)
                    }
                } else {
                    TypedValue::TimeZone(TimeZone::Utc)
                }
            },
            TypedValue::Cron(_) => {
                let len = 64;
                let mut buf = vec![0u8; (len as usize)+1];

                if let Some(str)= self.unwrap_and_log_esp(name, config_value, self.nvs_namespace.get_str(name, buf.as_mut_slice())) {
                    TypedValue::Cron(self.unwrap_and_log_cron(name, config_value, Cron::from_str(str)))
                    
                } else {
                    TypedValue::Cron(None)
                }
            },
        };
        info!("Finished reading config value {} from NVS: {:?}", name, result);
        config_value.value = result;
    }

    fn save(&self, name: &str, config_value: &mut ConfigSpecValue, str_val: &str) -> anyhow::Result<()> {
        
                log::info!("Config value {} is {}", name, str_val);
                match config_value.value.from_str(str_val) {
                    Ok(new_value) => {
                        if config_value.value.is_none() || new_value != config_value.value {
                            log::info!("Config value {} changed from {:?} to {:?}", name, config_value.value, new_value);

                            config_value.value = new_value;
                            // Save to NVS
                            log::info!("Save to NVS Config value {} is {}", name, str_val);
                            match &config_value.value {
                                TypedValue::String(len, Some(val)) => {
                                    info!("Saving string value for {} to NVS: {}", name, val);
                                    match self.nvs_namespace.set_str(name, val) {
                                        Ok(_) => info!("Saved OK Config value {} is {}", name, str_val),
                                        Err(error) => log::error!("Failed to save Config value {} is {}: {}", name, str_val, error),
                                    }

                                    {
                                        let mut buf = vec![0u8; (*len as usize)+1];

                                        if let Some(str)= self.unwrap_and_log_esp(name, config_value, self.nvs_namespace.get_str(name, buf.as_mut_slice())) {
                                            info!("Read back of {} is {}", name, str)
                                        } else {
                                            info!("Read back of {} is None", name)
                                        }
                                    }
                                },
                                TypedValue::Int32(Some(val)) => {
                                    info!("Saving int32 value for {} to NVS: {}", name, val);
                                    self.nvs_namespace.set_i32(name, *val)?
                                },
                                TypedValue::Int64(Some(val)) => {
                                    info!("Saving int64 value for {} to NVS: {}", name, val);
                                    self.nvs_namespace.set_i64(name, *val)?
                                },
                                TypedValue::Bool(val) => {
                                    info!("Saving bool value for {} to NVS: {}", name, val);
                                    self.nvs_namespace.set_u8(name, if *val { 1 } else { 0 })?
                                },
                                TypedValue::TimeZone(tz) => {
                                    info!("Saving TimeZone value for {} to NVS: {}", name, tz.to_str());
                                    self.nvs_namespace.set_str(name, tz.to_str())?
                                },
                                TypedValue::Cron(Some(val)) => {
                                    info!("Saving Cron value for {} to NVS: {}", name, val);
                                    self.nvs_namespace.set_str(name, val.as_str())?
                                },
                                _ => anyhow::bail!("Invalid config value for {}: {:?}", name, config_value.value),
                            };
                        }
                        else {
                            log::info!("Config value {} unchanged: {:?}", name, config_value.value);
                        }
                        Ok(())
                    }
                    Err(e) => {
                        anyhow::bail!("Failed to parse config value for {}: {}", name, e);
                    }
                }
    }
    
    fn remove(&self, name: &str, config_value: &mut ConfigSpecValue) -> anyhow::Result<()> {
        self.nvs_namespace.remove(name)?;
        config_value.value = config_value.value.to_none();
        if config_value.required {
            config_value.problem_id = self.problem_manager.set(config_value.problem_id, format!("Required value {} removed", name));
        }
        Ok(())
    }
    
    fn load_enabled_state(&self) -> anyhow::Result<EnabledState> {
        let enabled = if let Some(value) = self.feature_namespace.get_u8(&self.feature_name)? {
                info!("Read feature enabled value for {} from NVS: {}", &self.feature_name, value);
                value != 0
            } else {
                info!("Read feature enabled value for {} from NVS: None", &self.feature_name);
                false
            };

            Ok(EnabledState::from(enabled))
    }

    fn save_enabled_state(&self, enabled_state: EnabledState) -> anyhow::Result<()> {
        info!("Write feature enabled value for {} to NVS: {:?}", &self.feature_name, enabled_state);
        let value: u8 = if enabled_state.is_enabled() {1} else {0};
        self.feature_namespace.set_u8(&self.feature_name, value)?;
        Ok(())
    }
}