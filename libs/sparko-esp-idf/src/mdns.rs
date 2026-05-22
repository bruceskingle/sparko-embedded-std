use anyhow::Result;
use log::{error, info};
use simple_dns::{
    Name, Packet, CLASS, TYPE,
    ResourceRecord,
    rdata::{A, RData},
};
use std::{net::{Ipv4Addr, UdpSocket}, sync::mpsc::TryRecvError};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

pub struct MdnsResponder {
    wifi_receiver: Option<Receiver<Ipv4Addr>>,
}

impl MdnsResponder {
    pub fn new(wifi_receiver: Receiver<Ipv4Addr>) -> Self {
        Self { wifi_receiver: Some(wifi_receiver) }
    }

    fn run(wifi_receiver: Receiver<Ipv4Addr>, hostname: String) -> anyhow::Result<()>{

            let hostname_local = format!("{}.local", hostname);
            let domain_string = hostname_local.clone();
            let domain = Name::new_unchecked(&hostname_local);

            info!("mDNS waiting for IP...");
            let mut ip = wifi_receiver.recv()?;
            info!("mDNS IP acquired: {}", ip);

            let socket = UdpSocket::bind("0.0.0.0:5353")?;
            socket.set_nonblocking(false)?;

            // IMPORTANT for ESP32/lwIP: use actual interface IP
            socket.join_multicast_v4(
                &"224.0.0.251".parse().unwrap(),
                &ip,
            )?;

            socket.set_multicast_ttl_v4(255)?;

            let multicast_addr = "224.0.0.251:5353";

            // Pre-build A record (cloned per response)
            let mut base_record = ResourceRecord::new(
                domain.clone(),
                CLASS::IN,
                120,
                RData::A(A { address: ip.into() }),
            );


            info!("mDNS responder started: {} -> {}", hostname_local, ip);

            info!("Domain {}", &domain_string);
            let mut buf = [0u8; 1500];
            let mut last_announce = Instant::now();

            loop {
                //info!("mDNS responder top of loop");

                match wifi_receiver.try_recv() {
                    Ok(new_ip) => {
                        if new_ip != ip {
                            info!("mDNS IP updated: {} -> {}", ip, new_ip);

                            ip = new_ip;

                            // Rebuild A record
                            base_record = ResourceRecord::new(
                                domain.clone(),
                                CLASS::IN,
                                120,
                                RData::A(A { address: ip.into() }),
                            );

                            // Optional: immediate announcement (good practice)
                            let mut pkt = Packet::new_reply(0);
                            pkt.answers.push(base_record.clone());

                            if let Ok(bytes) = pkt.build_bytes_vec() {
                                let _ = socket.send_to(&bytes, multicast_addr);
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => {
                        // no update, continue normally
                    }
                    Err(TryRecvError::Disconnected) => {
                        error!("WiFi channel disconnected");
                    }
                }

                // --- 1. Handle incoming queries ---
                match socket.recv_from(&mut buf) {
                    Ok((len, src)) => {
                        //info!("mDNS responder got packet len {} from {}", len, src);
                        if let Ok(packet) = Packet::parse(&buf[..len]) {
                            if packet.questions.is_empty() {
                                //info!("mDNS responder got empty questions");
                                continue;
                            }

                            let mut should_reply = false;

                            for q in &packet.questions {
                                //info!("mDNS responder got question {:?}", q);
                                // Case-insensitive match
                                let name_match = q.qname.to_string()
                                    .eq_ignore_ascii_case(&hostname_local);

                                let type_match =
                                    q.qtype == simple_dns::QTYPE::TYPE(TYPE::A) || q.qtype == simple_dns::QTYPE::ANY;

                                //info!("mDNS responder name_match {} type_match {} q.qtype {:?}", name_match, type_match, q.qtype);

                                if name_match && type_match {
                                    should_reply = true;
                                    break;
                                }
                            }

                            //info!("mDNS responder should_reply {:?}", should_reply);

                            if should_reply {
                                let mut resp = Packet::new_reply(packet.id());

                                resp.answers.push(base_record.clone());

                                if let Ok(bytes) = resp.build_bytes_vec() {
                                    // Unicast response (fast path)
                                    if let Err(e) = socket.send_to(&bytes, src) {
                                        error!("mDNS send error: {}", e);
                                    } else {
                                        info!("mDNS response sent to {}", src);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("mDNS recv error: {}", e);
                    }
                }

                // --- 2. Occasional announcement (cache warmup) ---
                if last_announce.elapsed() > Duration::from_secs(300) {
                    let mut pkt = Packet::new_reply(0);
                    pkt.answers.push(base_record.clone());

                    if let Ok(bytes) = pkt.build_bytes_vec() {
                        let _ = socket.send_to(&bytes, multicast_addr);
                        info!("mDNS periodic announce: {}", hostname_local);
                    }

                    last_announce = Instant::now();
                }
            }
    }

    pub fn start(&mut self, hostname: String) -> Result<()> {

        if self.wifi_receiver.is_none() {
            anyhow::bail!("Receiver is None");
        }
        else {
            let wifi_receiver = std::mem::replace(
                &mut self.wifi_receiver,
                None, // dummy receiver
            ).unwrap();

            let hostname = hostname.clone();

            thread::spawn(move || {
                if let Err(error) = Self::run(wifi_receiver, hostname) {
                    error!("mDNS error: {}", error);
                }
            });

        }
        Ok(())
    }
}