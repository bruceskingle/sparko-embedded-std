// use embedded_svc::wifi::{Configuration, AuthMethod};
use embedded_svc::wifi::ClientConfiguration;
use esp_idf_hal::modem::WifiModemPeripheral;
use esp_idf_svc::wifi::AccessPointConfiguration;
use esp_idf_svc::wifi::AuthMethod;
use esp_idf_svc::wifi::Configuration;
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::ScanMethod;
use esp_idf_svc::wifi::ScanSortMethod;
use esp_idf_svc::wifi::WifiEvent;
use sparko_embedded_std::config::Config;
use sparko_embedded_std::problem::Problem;
use sparko_embedded_std::problem::ProblemManager;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::Sender;
use log::info;
use esp_idf_svc::handle::RawHandle;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspNvsPartition;
use esp_idf_svc::nvs::NvsDefault;

use crate::core::PASSWORD_LEN;
use crate::core::SSID_LEN;

pub struct WiFiManager<'a> {
    wifi: EspWifi<'a>,
    sys_loop: EspSystemEventLoop,
    wifi_sub: Arc<Mutex<Option<esp_idf_svc::eventloop::EspSubscription<'a, esp_idf_svc::eventloop::System>>>>,
    problem: Arc<Problem>,
    wifi_sender: Sender<Ipv4Addr>,
}

impl WiFiManager<'_> {
    pub fn new(
        modem: impl WifiModemPeripheral + 'static,
        sys_loop: EspSystemEventLoop,
        nvs: EspNvsPartition<NvsDefault>,
        problem_manager: &Arc<ProblemManager>,
        wifi_sender: Sender<Ipv4Addr>,
    ) -> anyhow::Result<Self> {
        let esp_wifi = EspWifi::new(modem, sys_loop.clone(), Some(nvs))?;

        Ok(Self {
            wifi: esp_wifi,
            sys_loop,
            wifi_sub: Arc::new(Mutex::new(None)),
            problem: Problem::new(problem_manager),
            wifi_sender,
        })
    }

    pub fn start_access_point(&mut self) -> anyhow::Result<std::net::Ipv4Addr> {
        

        let ap_config = AccessPointConfiguration {
            ssid: heapless::String::<SSID_LEN>::try_from("ESP32-Setup").unwrap(),
            password: heapless::String::<PASSWORD_LEN>::try_from("password").unwrap(),
            channel: 1,
            auth_method: AuthMethod::WPA2Personal,
            max_connections: 4,
            ..Default::default()
        };

        info!("Starting WiFi Access Point with config: {:?}", ap_config);

        self.wifi.set_configuration(&Configuration::AccessPoint(ap_config))?;

        // self.wifi.start().await?;

        let ip_info = self.wifi.ap_netif().get_ip_info()?;
        info!("WiFi Access Point IP Info: {:?}", ip_info);

        // Start the AP first, then update the DHCP server DNS option.
        self.wifi.start()?;

        // Desired DNS server for clients.
        let dns_server = ip_info.ip; //Ipv4Addr::new(1, 1, 1, 1);

        let dns_info = esp_idf_sys::esp_netif_dns_info_t {
            ip: esp_idf_sys::esp_ip_addr_t {
                type_: esp_idf_sys::lwip_ip_addr_type_IPADDR_TYPE_V4 as u8,
                u_addr: esp_idf_sys::_ip_addr__bindgen_ty_1 {
                    ip4: esp_idf_sys::esp_ip4_addr_t {
                        addr: u32::from_le_bytes(dns_server.octets()),
                    },
                },
            },
        };

        unsafe {
            let netif = self.wifi.ap_netif();
            let handle = netif.handle();

            let res_stop = esp_idf_sys::esp_netif_dhcps_stop(handle);
            if res_stop != esp_idf_sys::ESP_OK {
                log::warn!("esp_netif_dhcps_stop failed: {:?}", res_stop);
            }

            let res_set = esp_idf_sys::esp_netif_set_dns_info(
                handle,
                esp_idf_sys::esp_netif_dns_type_t_ESP_NETIF_DNS_MAIN,
                &dns_info as *const _ as *mut _,
            );
            if res_set != esp_idf_sys::ESP_OK {
                log::warn!("esp_netif_set_dns_info failed: {:?}", res_set);
            }

            let url_str = format!("http://{}/\0", ip_info.ip);
            let url = url_str.as_bytes();
            // let url = b"http://192.168.4.1/\0";

            esp_idf_sys::esp_netif_dhcps_option(
                handle,
                esp_idf_sys::esp_netif_dhcp_option_mode_t_ESP_NETIF_OP_SET,
                114,
                url.as_ptr() as *mut _,
                url.len() as u32
            );

            let res_start = esp_idf_sys::esp_netif_dhcps_start(handle);
            if res_start != esp_idf_sys::ESP_OK {
                log::warn!("esp_netif_dhcps_start failed: {:?}", res_start);
            }

            // Verify the DNS was set correctly
            let mut dns_out = esp_idf_sys::esp_netif_dns_info_t {
                ip: esp_idf_sys::esp_ip_addr_t {
                    type_: 0,
                    u_addr: esp_idf_sys::_ip_addr__bindgen_ty_1 {
                        ip4: esp_idf_sys::esp_ip4_addr_t { addr: 0 }
                    }
                }
            };
            let res_get = esp_idf_sys::esp_netif_get_dns_info(handle, esp_idf_sys::esp_netif_dns_type_t_ESP_NETIF_DNS_MAIN, &mut dns_out);
            if res_get == esp_idf_sys::ESP_OK {
                let addr_net = dns_out.ip.u_addr.ip4.addr;
                let octets = addr_net.to_be_bytes(); // network to host
                let retrieved_dns = Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]);
                log::info!("Retrieved DNS server: {:?}", retrieved_dns);
            } else {
                log::warn!("esp_netif_get_dns_info failed: {:?}", res_get);
            }
        }

        let ip_info = self.wifi.ap_netif().get_ip_info()?;
        info!("WiFi Access Point IP Info after dns config: {:?}", ip_info);

        Ok(ip_info.ip)
    }

    pub fn start_client(&mut self, config: &Config) -> anyhow::Result<std::net::Ipv4Addr> {
        let wifi_configuration: embedded_svc::wifi::Configuration = embedded_svc::wifi::Configuration::Client(ClientConfiguration {
            ssid: config.get_required_as_heapless::<32>(crate::core::SSID)?,
            bssid: None,
            auth_method: embedded_svc::wifi::AuthMethod::WPA2Personal,
            password: config.get_required_as_heapless::<64>(crate::core::WIFI_PASSWORD)?,
            channel: None,
            scan_method: ScanMethod::CompleteScan(ScanSortMethod::Security),
            pmf_cfg: esp_idf_svc::wifi::PmfConfiguration::Capable{ required: false },
        });


        self.wifi.set_configuration(&wifi_configuration)?;


        let problem = self.problem.clone();

        let wifi_sub: esp_idf_svc::eventloop::EspSubscription<'_, esp_idf_svc::eventloop::System> = self.sys_loop.subscribe::<WifiEvent, _>(move |event: WifiEvent| {
            match event {
                WifiEvent::Ready => info!("WiFi is Ready"),
                WifiEvent::ScanDone(sta_scan_done_ref) => info!("WiFi Scan Done: {:?} networks found", sta_scan_done_ref),
                WifiEvent::StaStarted => info!("WiFi Station Started"),
                WifiEvent::StaStopped => info!("WiFi Station Stopped"),
                WifiEvent::StaConnected(sta_connected_ref) => info!("WiFi Station Connected {:?}", sta_connected_ref),
                WifiEvent::StaDisconnected(sta_disconnected_ref) => {
                    info!("WiFi Station Disconnected {:?}", sta_disconnected_ref);
                    match sta_disconnected_ref.reason() as u32{
                        esp_idf_sys::wifi_err_reason_t_WIFI_REASON_AUTH_FAIL => {
                            info!("WiFi Station Authentication Failed");
                            problem.set("WiFi authentication failed. Please check your password.");
                        },
                        esp_idf_sys::wifi_err_reason_t_WIFI_REASON_4WAY_HANDSHAKE_TIMEOUT => {
                            info!("WiFi Station 4-Way Handshake Timeout");
                            problem.set("WiFi 4-way handshake timeout. Please check your password.");
                        },
                        esp_idf_sys::wifi_err_reason_t_WIFI_REASON_HANDSHAKE_TIMEOUT => {
                            info!("WiFi Station Handshake Timeout");
                            problem.set("WiFi handshake timeout. Please check your password");
                        },
                        esp_idf_sys::wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND => {
                            info!("WiFi Station No AP Found");
                            problem.set("WiFi no access point found. Please check your network settings and try again.");
                        },
                        _ => {}
                    }
                },
                WifiEvent::StaAuthmodeChanged => info!("WiFi Station Auth Mode Changed"),
                WifiEvent::StaBssRssiLow => info!("WiFi Station Bss Rssi Low"),
                WifiEvent::StaBeaconTimeout => info!("WiFi Station Beacon Timeout"),
                WifiEvent::StaWpsSuccess(wps_credentials_refs) => info!("WiFi Station Wps Success {:?}", wps_credentials_refs),
                WifiEvent::StaWpsFailed => info!("WiFi Station Wps Failed"),
                WifiEvent::StaWpsTimeout => info!("WiFi Station Wps Timeout"),
                WifiEvent::StaWpsPin(_) => info!("WiFi Station Wps Pin"),
                WifiEvent::StaWpsPbcOverlap => info!("WiFi Station Wps Pbc Overlap"),
                WifiEvent::ApStarted => info!("WiFi Access Point Started"),
                WifiEvent::ApStopped => info!("WiFi Access Point Stopped"),
                WifiEvent::ApStaConnected(ap_sta_connected_ref) => info!("WiFi Access Point Station Connected {:?}", ap_sta_connected_ref),
                WifiEvent::ApStaDisconnected(ap_sta_disconnected_ref) => info!("WiFi Access Point Station Disconnected {:?})", ap_sta_disconnected_ref),
                WifiEvent::ApProbeRequestReceived => info!("WiFi Access Point Probe Request Received"),
                WifiEvent::FtmReport => info!("WiFi Ftm Report"),
                WifiEvent::ActionTxStatus => info!("WiFi Action Tx Status"),
                WifiEvent::RocDone => info!("WiFi Roc Done"),
                WifiEvent::HomeChannelChange(home_channel_change) => info!("WiFi Home Channel Change {:?}", home_channel_change),
            }

            // if let WifiEvent::StaDisconnected(_) = event {
            //     panic!("WiFi disconnected");

            //     // // Drop server so it gets recreated
            //     // *server_clone.lock().unwrap() = None;

            //     // // Trigger reconnect
            //     // if let Ok(mut wifi) = wifi_clone.lock() {
            //     //     let _ = wifi.connect();
            //     // }
            // }
        })?;

        *self.wifi_sub.lock().unwrap() = Some(wifi_sub);

        info!("WiFi event subscription set up, starting WiFi...");

        self.wifi.start()?;
        info!("Wifi started");

        let mut retry_cnt = 4;
        while self.problem.is_clear() && retry_cnt > 0 {
            match self.wifi.connect() {
                Ok(_) => {
                    info!("Wifi connected");
                    break;
                },
                Err(error) => {
                    log::error!("Failed to connect to WiFi: {}", error);
                    retry_cnt -= 1;
                    if retry_cnt == 0 {
                        log::error!("Failed to connect to WiFi after multiple attempts");
                        return Err(error.into());
                    }
                    info!("Failed to connect to Wifi {} attempts remaining...", retry_cnt);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                },
            }
            
        }

        // Wait for IP (this replaces wait_netif_up)
        let ip_info;
        // while self.failure_reason.lock().unwrap().is_none() {
        loop {
            if self.problem.is_set() {
                log::error!("WiFi connection failed: {}", self.problem);
                return Err(anyhow::anyhow!(self.problem.to_string()));
            }

            if let Ok(info) = self.wifi.sta_netif().get_ip_info() {
                if info.ip != Ipv4Addr::UNSPECIFIED {
                    ip_info = info;
                    break;
                }
            }

            log::info!("Waiting for IP...");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        // let ip_info = self.wifi.sta_netif().get_ip_info()?;

        println!("Wifi DHCP info: {:?}", ip_info);
        
        // EspPing::default().ping(ip_info.subnet.gateway, &esp_idf_svc::ping::Configuration::default())?;

        self.wifi_sender.send(ip_info.ip)?;
        Ok(ip_info.ip)
    }
}
