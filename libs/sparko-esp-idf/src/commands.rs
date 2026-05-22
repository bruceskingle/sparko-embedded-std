use std::io::Write;

use indexmap::IndexMap;
use log::info;
use sparko_embedded_std::{command::Commands, config_manager::ConfigManager};

use crate::core::CORE_FEATURE_NAME;

pub struct EspCommands {
}

impl EspCommands {

    // fn erase_config(&self) -> anyhow::Result<()> {
    //     info!("Erasing config");
    //     if let Some(core_feature) = self.features.get(CORE_FEATURE_NAME) {
    //         core_feature.config_store.erase_all()?;
    //     }
    //     Ok(())
    // }
}

impl Commands for EspCommands {
    fn show_config_page(
        &self,
        resp: &mut dyn Write
        ) -> anyhow::Result<()>
    {
        resp.write(format!(r#"
                        <form method="POST" action="/command">
                        <label for="command">Command</label>
                            <select name="command" id="command">
                                <option value="restart">Restart</option>
                                <option value="factory_reset">Factory Reset</option>
                            </select>
                            <button type="submit">Execute</button>
                        </form>
                "#).as_bytes())?;
        Ok(())
    }
    
    fn handle_command(&self, resp: &mut dyn Write, form: IndexMap<String, String>, config_manager: &ConfigManager) -> anyhow::Result<()> {
        let command =form.get("command");
            match command.map(|s| s.as_str()) {
                Some("restart") => {
                    info!("Restart command received, restarting...");
                    resp.write(b"<!doctype html><html><head><meta http-equiv=\"refresh\" content=\"5;url=/\" /><title>Restarting</title></head><body><p>Device restarting, redirecting to root in 5 seconds...</p><script>setTimeout(()=>{window.location.href='/';},5000);</script></body></html>")?;

                    std::thread::spawn(|| {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        unsafe { esp_idf_sys::esp_restart(); }
                    });
                },
                Some("factory_reset") => {
                    info!("Factory reset command received, erasing config and restarting...");
                    if let Err(e) = config_manager.erase_config(CORE_FEATURE_NAME) {
                        log::error!("Failed to erase config: {}", e);
                        resp.write(b"<!doctype html><html><head><meta http-equiv=\"refresh\" content=\"5;url=/\" /><title>Factory reset failed</title></head><body><p>Failed to erase config.</p><script>setTimeout(()=>{window.location.href='/';},5000);</script></body></html>")?;
                    }
                    else {
                        resp.write(b"<!doctype html><html><head><meta http-equiv=\"refresh\" content=\"5;url=/\" /><title>Factory reset</title></head><body><p>Config erased. Device restarting, redirecting to root in 5 seconds...</p><script>setTimeout(()=>{window.location.href='/';},5000);</script></body></html>")?;
                    
                        std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            unsafe { esp_idf_sys::esp_restart(); }
                        });
                    }
                },
                Some(cmd) => {
                    log::warn!("Unknown command received: {}", cmd);
                        resp.write(format!("Unknown command received: {}", cmd).as_bytes())?;
                },
                None => {
                    log::warn!("No command received in form");
                        resp.write(b"No command received in form")?;
                }
            }

            Ok(())
    }
}