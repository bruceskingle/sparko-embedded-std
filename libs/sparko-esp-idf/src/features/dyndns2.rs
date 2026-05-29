use std::net::IpAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use croner::Cron;
use esp_idf_svc::http::Method;
use esp_idf_svc::http::client::EspHttpConnection;
use log::info;
use sparko_embedded_std::config::ConfigSpec;
use sparko_embedded_std::config::ConfigSpecValue;
use sparko_embedded_std::config::FeatureConfig;
use sparko_embedded_std::config::TypedValue;
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;

use crate::Esp32Platform;
use crate::Esp32PlatformInitializer;
use crate::{Feature, FeatureDescriptor};

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
