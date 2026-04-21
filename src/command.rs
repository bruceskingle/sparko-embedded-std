use std::io::Write;

use indexmap::IndexMap;

use crate::config_manager::ConfigManager;

pub trait Commands: Sync + Send {
    fn show_config_page(
        &self,
        resp: &mut dyn Write
        ) -> anyhow::Result<()>;
    
    fn handle_command(&self, resp: &mut dyn Write, form: IndexMap<String, String>, config_manager: &ConfigManager) -> anyhow::Result<()>;
}