use std::net::IpAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use croner::Cron;
use esp_idf_svc::http::Method;
use esp_idf_svc::http::client::EspHttpConnection;
use log::info;
use sparko_embedded_std::config::Config;
use sparko_embedded_std::config::ConfigSpec;
use sparko_embedded_std::config::ConfigSpecValue;
use sparko_embedded_std::config::FeatureConfig;
use sparko_embedded_std::config::TypedValue;
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;

use crate::Esp32Platform;
use crate::Esp32PlatformInitializer;
use crate::{Feature, FeatureDescriptor};

//                                           123456789012345<-------- Max Name Length 15
pub const USER_NAME: &str = "user_name";
pub const PASSWORD: &str = "password";
pub const HOSTNAME: &str = "hostname";
pub const BASE_SERVICE_URL: &str = "base_url";
pub const GET_IP_URL: &str = "get_ip_url";
pub const GET_REQUIRES_STRIP: &str = "get_req_strip";
pub const UPDATE_URL: &str = "update_url";
pub const UPDATE_REQUIRES_ADDRESS: &str = "upd_req_addr";
pub const UPDATE_INTERVAL: &str = "upd_int";
pub const SCHEDULE: &str = "schedule";

// pub struct DynDns2Config {
//     user_name: String,
//     password: String,
//     hostname: String,
//     base_service_url: String,
//     get_ip_url: Option<String>,
//     get_requires_strip: bool,
//     update_url: Option<String>,
//     update_requires_address: bool,
//     update_interval: u64,
// }

// impl DynDns2Config {
//     pub fn new(config_manager: &ConfigManager) -> anyhow::Result<Self> {
//         Ok(Self {
//             user_name: config_manager.get(USER_NAME)?.unwrap_or_default(),
//             password: config_manager.get(PASSWORD)?.unwrap_or_default(),
//             hostname: config_manager.get(HOSTNAME)?.unwrap_or_default(),
//             base_service_url: config_manager.get(BASE_SERVICE_URL)?.unwrap_or_default(),
//             get_ip_url: config_manager.get(GET_IP_URL)?,
//             get_requires_strip: config_manager.get(GET_REQUIRES_STRIP)?.unwrap_or(false),
//             update_url: config_manager.get(UPDATE_URL)?,
//             update_requires_address: config_manager.get(UPDATE_REQUIRES_ADDRESS)?.unwrap_or(false),
//             update_interval: config_manager.get(UPDATE_INTERVAL)?.unwrap_or(3600),
//         })
//     }
// }

#[derive(FeatureConfig)]
pub struct DynDns2Config {
    #[config(len = 32)]
    user_name: String,
    #[config(len = 32)]
    password: String,
    #[config(len = 64)]
    hostname: String,
    #[config(len = 64)]
    get_ip_url: String,
    #[config(len = 64)]
    update_url: String,
    upd_req_address: bool,
    schedule: Cron,
}

pub struct DynDns2 {}

impl DynDns2 {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

impl Feature for DynDns2 {
    type Config = DynDns2Config;

    fn init(
        &self,
        _initializer: &mut crate::Esp32PlatformInitializer,
    ) -> anyhow::Result<FeatureDescriptor> {
        info!("DynDns2::init()");
        // let config = ConfigSpec::builder()
        //     .with(
        //         USER_NAME.to_string(),
        //         ConfigSpecValue::new(TypedValue::String(32, None), true),
        //     )?
        //     .with(
        //         PASSWORD.to_string(),
        //         ConfigSpecValue::new(TypedValue::String(32, None), true),
        //     )?
        //     .with(
        //         HOSTNAME.to_string(),
        //         ConfigSpecValue::new(TypedValue::String(64, None), true),
        //     )?
        //     // .with(BASE_SERVICE_URL.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
        //     .with(
        //         GET_IP_URL.to_string(),
        //         ConfigSpecValue::new(TypedValue::String(64, None), true),
        //     )?
        //     // .with(GET_REQUIRES_STRIP.to_string(), ConfigSpecValue::new(TypedValue::Bool(false), false))?
        //     .with(
        //         UPDATE_URL.to_string(),
        //         ConfigSpecValue::new(TypedValue::String(64, None), true),
        //     )?
        //     .with(
        //         UPDATE_REQUIRES_ADDRESS.to_string(),
        //         ConfigSpecValue::new(TypedValue::Bool(None), false),
        //     )?
        //     .with(
        //         SCHEDULE.to_string(),
        //         ConfigSpecValue::new(TypedValue::Cron(None), true),
        //     )?
        //     .build();

        Ok(FeatureDescriptor {
            name: "DynDNS2".to_string(),
            config: DynDns2Config::to_config_spec()?,
        })
    }

    fn start(
        &mut self,
        _sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: DynDns2Config,
    ) -> anyhow::Result<()> {
        let schedule_spec = config.schedule.pattern.to_string();
        let resolve_task = ResolveTask::new(config)?;

        initializer.add_task(Box::new(resolve_task), &schedule_spec)?;
        Ok(())
    }
}

pub struct ResolveTask {
    host_name: String,
    user_name: String,
    password: String,
    get_ip_url: String,
    update_url: String,
    addr: Arc<Mutex<IpAddr>>,
    cnt: u32,
}

impl ScheduledTask<Esp32Platform> for ResolveTask {
    fn run(&mut self, _sparko_cyd: &mut Esp32Platform) -> anyhow::Result<()> {
        self.execute()
    }

    fn name(&self) -> &str {
        "DynDns2 Resolver"
    }
}

impl ResolveTask {
    pub fn new(config: DynDns2Config) -> anyhow::Result<Self> {
        // log::info!("Trace 3");
        // let host_name = config.get_valid(HOSTNAME)?;
        // let user_name = config.get_valid(USER_NAME)?;
        // let password = config.get_valid(PASSWORD)?;
        // let update_url = config.get_valid(UPDATE_URL)?;
        // let get_ip_url = config.get_valid(GET_IP_URL)?;

        // let http_client = embedded_svc::http::client::Client::wrap(EspHttpConnection::new(&esp_idf_svc::http::client::Configuration {
        //     // use_global_ca_store: true,
        //     crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        //     ..Default::default()
        // })?);

        let current_dns = Self::resolve_single(&config.hostname)?;
        info!(
            "Current DNS resolution for {}: {}",
            &config.hostname, current_dns
        );

        let addr = Arc::new(Mutex::new(current_dns));

        let mut task = Self {
            host_name: config.hostname,
            user_name: config.user_name,
            password: config.password,
            get_ip_url: config.get_ip_url,
            update_url: config.update_url,
            addr,
            cnt: 0,
            // http_client,
        };

        task.execute()?;

        Ok(task)
    }

    fn execute(&mut self) -> anyhow::Result<()> {
        match self.get_public_ip_address() {
            Ok(public_ip) => {
                self.cnt = self.cnt + 1;
                if public_ip != *self.addr.clone().lock().unwrap() {
                    log::info!(
                        "Public IP changed: {} -> {}",
                        *self.addr.lock().unwrap(),
                        public_ip
                    );
                    // *self.addr.lock()? = public_ip;
                    let url = format!(
                        "{}?username={}&password={}&hostname={}",
                        self.update_url, self.user_name, self.password, self.host_name
                    );

                    self.get_ignore_response_body(&url)?;
                } else {
                    log::info!("Public IP unchanged: {}", public_ip);
                }
            }
            Err(e) => {
                log::error!("Failed to get public IP address: {}", e);
            }
        }

        Ok(())
    }

    fn resolve_single(name: &str) -> anyhow::Result<IpAddr> {
        let addr = (name, 0)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow::anyhow!("DNS returned no addresses"))?;

        Ok(addr.ip())
    }

    fn get_public_ip_address(&mut self) -> anyhow::Result<IpAddr> {
        let mut http_client = embedded_svc::http::client::Client::wrap(EspHttpConnection::new(
            &esp_idf_svc::http::client::Configuration {
                // use_global_ca_store: true,
                crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
                ..Default::default()
            },
        )?);

        info!(
            "DynDns2: about to get_public_ip_address from: {}",
            &self.get_ip_url
        );
        let request = http_client.request(Method::Get, &self.get_ip_url, &[])?;
        let mut response = request.submit()?;

        info!(
            "DynDns2: get_public_ip_address Status: {}",
            response.status()
        );

        let mut body = [0u8; 512];
        let bytes_read = response.read(&mut body)?;

        let html = core::str::from_utf8(&body[..bytes_read])
            .unwrap_or("invalid utf8")
            .trim();

        let start = html.find("<body>").map(|i| i + "<body>".len()).unwrap_or(0);

        let end = html[start..]
            .find("</body>")
            .map(|i| start + i)
            .unwrap_or(html.len());

        let raw_addr_str = &html[start..end];

        // remove anything up to and including the final space
        let addr_str = match raw_addr_str.rfind(' ') {
            Some(idx) => &raw_addr_str[idx + 1..],
            None => raw_addr_str,
        };

        info!("get IP result raw={} truncated={}", raw_addr_str, addr_str);

        let addr: IpAddr = IpAddr::from_str(addr_str)?;

        // println!(
        //     "Body: {}",
        //     addr_str
        // );
        // println!(
        //     "IP Address: {}",
        //     addr
        // );
        Ok(addr)
    }

    fn get_ignore_response_body(&mut self, url: &str) -> anyhow::Result<()> {
        let mut http_client = embedded_svc::http::client::Client::wrap(EspHttpConnection::new(
            &esp_idf_svc::http::client::Configuration {
                // use_global_ca_store: true,
                crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
                ..Default::default()
            },
        )?);

        let request = http_client.request(Method::Get, url, &[])?;
        let mut response = request.submit()?;

        println!("Status: {}", response.status());

        let mut body = [0u8; 512];
        let bytes_read = response.read(&mut body)?;

        let response = core::str::from_utf8(&body[..bytes_read])
            .unwrap_or("invalid utf8")
            .trim();

        println!("Body: {}", response);
        // println!(
        //     "IP Address: {}",
        //     addr
        // );
        Ok(())
    }
}
