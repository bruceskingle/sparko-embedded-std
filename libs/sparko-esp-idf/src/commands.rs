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

/// A page saying what happened, styled as the others, which goes back to the home page after 5 seconds.
fn message_page(resp: &mut dyn Write, title: &str, message: &str) -> anyhow::Result<()> {
    resp.write(format!(r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<meta http-equiv="refresh" content="5;url=/" />
<title>{title}</title>
<link rel="stylesheet" href="/main.css">
</head>
<body><div class="page"><h1>{title}</h1><p>{message}</p><p><a href="/">Home</a></p></div></body>
</html>"#).as_bytes())?;
    Ok(())
}

impl Commands for EspCommands {
    fn show_config_page(
        &self,
        resp: &mut dyn Write
        ) -> anyhow::Result<()>
    {
        resp.write(format!(r#"
                        <section class="feature">
                        <h2>Commands</h2>
                        <form method="POST" action="/command">
                        <label for="command">Command</label>
                            <select name="command" id="command">
                                <option value="restart">Restart</option>
                                <option value="factory_reset">Factory Reset</option>
                            </select>
                            <button type="submit">Execute</button>
                        </form>
                        </section>
                "#).as_bytes())?;
        Ok(())
    }
    
    fn handle_command(&self, resp: &mut dyn Write, form: IndexMap<String, String>, config_manager: &ConfigManager) -> anyhow::Result<()> {
        let command =form.get("command");
            match command.map(|s| s.as_str()) {
                Some("restart") => {
                    info!("Restart command received, restarting...");
                    message_page(resp, "Restarting", "The device is restarting, this page goes to its home page in 5 seconds.")?;

                    // A small stack, as the default may not fit in the RAM which is left.
                    std::thread::Builder::new().stack_size(4096).spawn(|| {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        unsafe { esp_idf_sys::esp_restart(); }
                    })?;
                },
                Some("factory_reset") => {
                    info!("Factory reset command received, erasing config and restarting...");
                    if let Err(e) = config_manager.erase_config(CORE_FEATURE_NAME) {
                        log::error!("Failed to erase config: {}", e);
                        message_page(resp, "Factory reset failed", "The settings could not be erased.")?;
                    }
                    else {
                        message_page(resp, "Factory reset", "The settings have been erased and the device is restarting, with its own Wi-Fi network for setting it up again.")?;
                    
                        // A small stack, as the default may not fit in the RAM which is left.
                        std::thread::Builder::new().stack_size(4096).spawn(|| {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            unsafe { esp_idf_sys::esp_restart(); }
                        })?;
                    }
                },
                Some(cmd) => {
                    log::warn!("Unknown command received: {}", cmd);
                        message_page(resp, "Unknown command", &format!("There is no command {cmd}."))?;
                },
                None => {
                    log::warn!("No command received in form");
                        message_page(resp, "No command", "No command was chosen.")?;
                }
            }

            Ok(())
    }
}