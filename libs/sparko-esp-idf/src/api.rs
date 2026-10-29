//! The device's API, for an app, e.g. Marco Sparko, to find out what the device is and manage it, over HTTPS with the
//! device's own certificate, see identity.rs, which the app pins. It runs alongside the HTTP server of the /config page
//! and captive portal, on port 443, once the device is on the Wi-Fi.
//!
//! - GET /api/info: what the device is, see info.
//! - POST /api/adopt/start, then POST /api/adopt {"code", "key"}: adoption, see adoption.rs.
//! - POST /api/release
//! - GET /api/config: every feature's config, see ConfigManager::to_json, with secrets only as set or not.
//! - POST /api/config: changes it, see ConfigManager::apply_json, which takes effect after a restart.
//! - POST /api/restart
//!
//! Features add their own, e.g. the data they capture, with on. Once the device has been adopted, every request but
//! /api/info and adoption's needs the adopting app's key, see Adoption::is_authorised.

use std::sync::Arc;

use esp_idf_svc::http::Method;
use esp_idf_svc::http::server::{Configuration, EspHttpConnection, EspHttpServer, Request};
use esp_idf_svc::io::Write;
use esp_idf_svc::nvs::{EspNvsPartition, NvsDefault};
use esp_idf_svc::tls::X509;
use log::info;
use serde_json::{Value, json};
use sparko_embedded_std::config_manager::ConfigManager;

use crate::Firmware;
use crate::adoption::{Adoption, PAIRING_TIME};
use crate::identity::DeviceIdentity;

/// The largest request body read, to protect the RAM.
const MAX_BODY: usize = 8 * 1024;

pub struct ApiServer {
    server: EspHttpServer<'static>,
    identity: Arc<DeviceIdentity>,
    adoption: Arc<Adoption>,
}

impl ApiServer {
    pub(crate) fn start(
        nvs_partition: &EspNvsPartition<NvsDefault>,
        config_manager: Arc<ConfigManager>,
        adoption: Arc<Adoption>,
        firmware: Firmware,
    ) -> anyhow::Result<ApiServer> {
        let identity = Arc::new(DeviceIdentity::load_or_create(nvs_partition)?);
        let mut server = EspHttpServer::new(&Configuration {
            // Its own control port, as the HTTP server has the default one.
            ctrl_port: 32769,
            // Each HTTPS connection needs about 30 KB while it is open, see sdkconfig.defaults.
            max_open_sockets: 2,
            server_certificate: Some(X509::pem(identity.cert_pem)),
            private_key: Some(X509::pem(identity.key_pem)),
            ..Default::default()
        })?;

        let info_identity = identity.clone();
        let info_config = config_manager.clone();
        let info_adoption = adoption.clone();
        server.fn_handler("/api/info", Method::Get, move |request| {
            respond(request, 200, &info(&info_identity, &info_config, &info_adoption, firmware))
        })?;

        let start_adoption = adoption.clone();
        server.fn_handler("/api/adopt/start", Method::Post, move |request| match start_adoption.start_pairing() {
            Ok(time) => respond(request, 200, &json!({ "expiresInSeconds": time.as_secs() })),
            Err(error) => respond(request, 409, &json!({ "error": format!("{error:#}") })),
        })?;

        let adopt_adoption = adoption.clone();
        server.fn_handler("/api/adopt", Method::Post, move |mut request| {
            let result = read_json(&mut request).and_then(|body| {
                let text = |name: &str| body.get(name).and_then(|value| value.as_str()).unwrap_or_default().to_string();
                adopt_adoption.adopt(&text("code"), &text("key"))
            });
            match result {
                Ok(()) => respond(request, 200, &json!({ "adopted": true })),
                Err(error) => respond(request, 403, &json!({ "error": format!("{error:#}") })),
            }
        })?;

        let release_adoption = adoption.clone();
        server.fn_handler("/api/release", Method::Post, move |request| {
            if !authorised(&release_adoption, &request) {
                return unauthorised(request);
            }
            match release_adoption.release() {
                Ok(()) => respond(request, 200, &json!({ "adopted": false })),
                Err(error) => respond(request, 500, &json!({ "error": format!("{error:#}") })),
            }
        })?;

        let get_config = config_manager.clone();
        let get_adoption = adoption.clone();
        server.fn_handler("/api/config", Method::Get, move |request| {
            if !authorised(&get_adoption, &request) {
                return unauthorised(request);
            }
            respond(request, 200, &get_config.to_json())
        })?;

        let post_config = config_manager;
        let post_adoption = adoption.clone();
        server.fn_handler("/api/config", Method::Post, move |mut request| {
            if !authorised(&post_adoption, &request) {
                return unauthorised(request);
            }
            let update = match read_json(&mut request) {
                Ok(update) => update,
                Err(error) => return respond(request, 400, &json!({ "error": format!("{error:#}") })),
            };
            match post_config.apply_json(&update) {
                Ok(()) => {
                    info!("Config changed through the API");
                    respond(request, 200, &json!({ "restartRequired": true, "config": post_config.to_json() }))
                }
                Err(error) => respond(request, 400, &json!({ "error": format!("{error:#}") })),
            }
        })?;

        let restart_adoption = adoption.clone();
        server.fn_handler("/api/restart", Method::Post, move |request| {
            if !authorised(&restart_adoption, &request) {
                return unauthorised(request);
            }
            respond(request, 200, &json!({ "restarting": true }))?;
            // After the response has gone.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(1));
                unsafe { esp_idf_sys::esp_restart() };
            });
            Ok::<(), anyhow::Error>(())
        })?;

        info!("API server started on port 443, pairing codes last {} s", PAIRING_TIME.as_secs());
        Ok(ApiServer { server, identity, adoption })
    }

    pub fn identity(&self) -> &DeviceIdentity {
        &self.identity
    }

    pub fn adoption(&self) -> &Arc<Adoption> {
        &self.adoption
    }

    /// Adds a handler, e.g. for a feature's data, whose uri should start /api/, which, once the device has been
    /// adopted, is only called for requests with the adopting app's key.
    pub fn on<F>(&mut self, uri: &str, method: Method, f: F) -> anyhow::Result<&mut Self>
    where
        F: for<'r> Fn(Request<&mut EspHttpConnection<'r>>) -> anyhow::Result<()> + Send + 'static,
    {
        let adoption = self.adoption.clone();
        self.server.fn_handler(uri, method, move |request| {
            if !authorised(&adoption, &request) {
                return unauthorised(request);
            }
            f(request)
        })?;
        Ok(self)
    }
}

fn authorised(adoption: &Adoption, request: &Request<&mut EspHttpConnection<'_>>) -> bool {
    adoption.is_authorised(request.header("Authorization"))
}

fn unauthorised(request: Request<&mut EspHttpConnection<'_>>) -> anyhow::Result<()> {
    respond(request, 401, &json!({ "error": "the device has been adopted, and this needs the adopting app's key" }))
}

/// What the device is: its ID, board, firmware, certificate fingerprint, whether it has been adopted, and its features.
fn info(identity: &DeviceIdentity, config_manager: &ConfigManager, adoption: &Adoption, firmware: Firmware) -> Value {
    let features: Vec<Value> = config_manager
        .features
        .values()
        .map(|feature| json!({ "name": feature.name, "enabled": feature.inner.lock().unwrap().enabled.is_enabled() }))
        .collect();
    json!({
        "id": identity.id,
        "board": crate::board_name(),
        "firmware": { "name": firmware.name, "version": firmware.version },
        "platform": { "name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION") },
        "certificateFingerprint": identity.fingerprint,
        "adopted": adoption.is_adopted(),
        "upTimeSeconds": unsafe { esp_idf_sys::esp_timer_get_time() } / 1_000_000,
        "features": features,
    })
}

fn read_json(request: &mut Request<&mut EspHttpConnection<'_>>) -> anyhow::Result<Value> {
    let mut body = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        let read = request.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if body.len() + read > MAX_BODY {
            anyhow::bail!("the request is longer than {MAX_BODY} bytes");
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(serde_json::from_slice(&body)?)
}

pub fn respond(request: Request<&mut EspHttpConnection<'_>>, status: u16, json: &Value) -> anyhow::Result<()> {
    let mut response = request.into_response(status, None, &[("Content-Type", "application/json")])?;
    response.write_all(&serde_json::to_vec(json)?)?;
    Ok(())
}
