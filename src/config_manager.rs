
use std::{io::Write, sync::{Arc, Mutex}};

use indexmap::IndexMap;
use log::info;

use crate::{command::Commands, config::{Config, ConfigStoreFactory}, feature::{FeatureConfig, FeatureDescriptor}, http_server::{HttpMethod, HttpServerManager}, problem::ProblemManager};




pub struct ConfigManagerBuilder {
    config_store_factory: Box<dyn ConfigStoreFactory>,
    features: IndexMap<String, FeatureConfig>,
    problem_manager: Arc<ProblemManager>,
    ap_mode: Arc<Mutex<bool>>,
    commands: Box<dyn Commands>,
}

impl ConfigManagerBuilder {
    fn new(
        config_store_factory: Box<dyn ConfigStoreFactory>,
        problem_manager: Arc<ProblemManager>,
        ap_mode: Arc<Mutex<bool>>,
        commands: Box<dyn Commands>,
    ) -> anyhow::Result<Self>
    {
        let features: IndexMap<String, FeatureConfig> = IndexMap::new();

        Ok(Self {
            config_store_factory,
            features,
            problem_manager,
            ap_mode,
            commands,
        })
    }

    pub fn add_feature(&mut self, descriptor: FeatureDescriptor, internal: bool) -> anyhow::Result<(Config, bool)> {
        log::info!("About to create config for feature: {}", &descriptor.name);
        let feature_config = FeatureConfig::from_feature(descriptor, &self.config_store_factory, internal)?;
        let feature_name = feature_config.name.clone();
        let config = feature_config.to_config();
        let valid = feature_config.is_valid();
        log::info!("Added feature: {} valid={}", &feature_name, &valid);

        self.features.insert(feature_name, feature_config);

        log::info!("List ConfigManager:");
        for name in self.features.keys() {
            log::info!("Current feature in ConfigManager: {}", name);
        }
        log::info!("END List ConfigManager:");

        Ok((config, valid))
    }

    pub fn build(self) -> ConfigManager {
        ConfigManager {
            features: self.features,
            problem_manager: self.problem_manager,
            ap_mode: self.ap_mode,
            commands: self.commands,
        }
    }
}

pub struct ConfigManager {
    pub features: IndexMap<String, FeatureConfig>,
    problem_manager: Arc<ProblemManager>,
    ap_mode: Arc<Mutex<bool>>,
    commands: Box<dyn Commands>,
}

impl ConfigManager {
    pub fn builder(
        config_store_factory: Box<dyn ConfigStoreFactory>,
        problem_manager: Arc<ProblemManager>,
        ap_mode: Arc<Mutex<bool>>,
        commands: Box<dyn Commands>
    )  -> anyhow::Result<ConfigManagerBuilder>
    {
        ConfigManagerBuilder::new(config_store_factory, problem_manager, ap_mode, commands)
    }

    pub fn erase_config(&self, feature_name: &str) -> anyhow::Result<()> {
        info!("Erasing config");
        if let Some(core_feature) = self.features.get(feature_name) {
            core_feature.config_store.erase_all()?;
        }
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        for (_feature_name, feature_config) in &self.features {
            if ! feature_config.is_valid() {
                return false;
            }
        }
        info!("ConfigManager is valid");
        true
    }

    pub fn is_online(&self) -> bool {
        let ap_mode = *self.ap_mode.lock().unwrap();
        info!("is_ap_mode: {}", ap_mode);
        !ap_mode
    }

    fn show_config_page(config_manager_clone: &Arc<ConfigManager>, 
        resp: &mut dyn Write
        ) -> anyhow::Result<()> {
            resp.write(r#"
                <!DOCTYPE html>
                <html lang="en">
                <head>
                    <meta charset="utf-8" />
                    <meta name="viewport" content="width=device-width, initial-scale=1" />
                    <title>ESP32 Setup</title>
                    <link rel="stylesheet" href="/main.css">
                </head>
                <body>
                    <div class="page">"#.as_bytes())?;
            
            

            if ! config_manager_clone.problem_manager.is_empty() {
                info!("Failure reason present, showing error message on config page");
                resp.write(format!(r#"
                    <div style="background: #ffdddd; border: 1px solid #ff5c5c; padding: 10px; margin-bottom: 18px; border-radius: 8px;">
                        <strong>Error:</strong> <ul>
                "#).as_bytes())?;

                for reason in &*config_manager_clone.problem_manager {
                    resp.write("<li>".as_bytes())?;
                    resp.write(reason.as_bytes())?;
                    resp.write("</li>\n".as_bytes())?;
                }

                resp.write(format!(r#"
                    </ul>
                    </div>
                "#).as_bytes())?;
            }
            else {
                info!("No failure reason, not showing error message on config page");
            }
            resp.write(r#"
                        <h1>ESP32 Setup</h1>
                        <form method="POST" action="/update_config">"#.as_bytes())?;
            for (_feature_name, feature_config) in &config_manager_clone.features {
                feature_config.create_config_page(resp)?;
            }

            
            resp.write(format!(r#"<button type="submit">Save</button>
                        </form>
                "#).as_bytes())?;
            
            config_manager_clone.commands.show_config_page(resp)?;
            
            resp.write(format!(r#"
                    </div>
                </body>
                </html>
                "#).as_bytes())?;
            Ok(())
    }

    fn handle_command(&self, resp: &mut dyn Write, form: IndexMap<String, String>) -> anyhow::Result<()> {
        self.commands.handle_command(resp, form, &self)
    }

    pub fn create_pages(
        config_manager: &Arc<ConfigManager>,
        server_manager: &mut dyn HttpServerManager
    ) -> anyhow::Result<()> {
        let config_manager_clone = config_manager.clone();

        server_manager.handle("/config", HttpMethod::Get, Box::new(move |resp| {
            Self::show_config_page(&config_manager_clone, resp)
        }))?;

        let config_manager_clone = config_manager.clone();

        server_manager.handle_post_form("/command", Box::new(move |resp, form| {
            config_manager_clone.handle_command(resp, form)
        }))?;

        let config_manager_clone = config_manager.clone();

        server_manager.handle_post_form("/update_config", Box::new(move | resp, form | {
            config_manager_clone.handle_config_form(&form)?;
            Self::show_config_page(&config_manager_clone, resp)
        }))?;

        Ok(())
    }

    pub fn handle_config_form(&self, form: &IndexMap<String, String>) -> anyhow::Result<()> {
        info!("Handling config form submission: {:?}", form);
        for (_feature_name, feature_config) in &self.features {
            feature_config.handle_config_form(form)?;
        }
        Ok(())
    }
}