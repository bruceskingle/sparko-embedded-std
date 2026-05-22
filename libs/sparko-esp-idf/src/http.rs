use std::{io::Write, sync::Arc};

use esp_idf_svc::http::{Method, server::{EspHttpConnection, EspHttpServer}};
use indexmap::IndexMap;
use log::info;
use sparko_platform::http_server::{HttpMethod, HttpServerManager};
use url::form_urlencoded;

/*
The abstraction we have here provides a couple of wrapper methods which make it possible for code which is not dependent
on esp_idf_svc to create web pages which return a fixed status and headers and provide a closure to generate the output body.
Anything which requires conditional HTTP status codes or the like needs to be implemented using this type directly.

I have made extensive efforts to refactor this into a hardware agnostic set of traits and an ESP32 specific back end
without success. Neither boxed trait objects (which require memory allocations on each request) or a solution based
on generic associated types (GAT) will work.

My conclusion is that this is not possible between the way the ESP Http server works and the limitations of the rust
type system.

The code for the attempted rewrite is in http_server.rs and esp_http_server.rs in the http_api branch.

I HAVE BEEN DOWN THIS RABBIT HOLE TWICE NOW.

NO ENTRY.

STOP.

GO BACK.
*/


fn to_esp_method(http_method: HttpMethod) -> Method {
    match http_method {
        HttpMethod::Get => Method::Get,
        HttpMethod::Post => Method::Post,
    }
}

pub struct WriteWrapper<'r, 'c> {
    pub resp: esp_idf_svc::http::server::Response<&'r mut EspHttpConnection<'c>>,
}

impl<'r, 'c> std::io::Write for WriteWrapper<'r, 'c> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.resp.write(buf) {
            Ok(result) => Ok(result),
            Err(error) => Err(std::io::Error::new(std::io::ErrorKind::Other, error)),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.resp.flush() {
            Ok(_) => Ok(()),
            Err(error) => Err(std::io::Error::new(std::io::ErrorKind::Other, error)),
        }
    }
}

pub struct EspHttpServerManager<'a>{
    server: EspHttpServer<'a>,
}



fn is_online(ap_mode: &Arc<std::sync::Mutex<bool>>) -> bool {
    let is_ap_mode = *ap_mode.lock().unwrap();
    info!("is_ap_mode: {}", is_ap_mode);
    !is_ap_mode
}

impl EspHttpServerManager<'_> {
    pub fn new() -> anyhow::Result<Self> {
        let server = EspHttpServer::new(&Default::default())?;
        Ok(Self {
            server,
        })
    }

    pub fn on<F>(
        &mut self,
        uri: &str,
        method: Method,
        f: F,
    ) -> anyhow::Result<&mut Self>
    where
        F: for<'r> Fn(esp_idf_svc::http::server::Request<&mut EspHttpConnection<'r>>) -> anyhow::Result<()> + Send + 'static,
    {
        self.server.fn_handler(uri, method, f)?;

        Ok(self)
    }

    pub fn init_captive_portal(
        &mut self,
        ap_mode: &Arc<std::sync::Mutex<bool>>
    ) -> anyhow::Result<()> {
        let ap_mode_clone = ap_mode.clone();

        self.on("/generate_204", Method::Get, move |req| {
            info!("Received {:?} request for {} configured={}", req.method(), req.uri(), is_online(&ap_mode_clone));
            if is_online(&ap_mode_clone) { 
                let mut resp = req.into_ok_response()?;        
                resp.write(b"<HTML><BODY>Success</BODY></HTML>")?;
            } else {
                let mut resp = req.into_response(302, None, &[("Location", "/config")])?;
                resp.write(b"<HTML><BODY>Not configured</BODY></HTML>")?;
            }
            Ok(())
        })?;

        let ap_mode_clone = ap_mode.clone();
        self.on("/hotspot-detect.html", Method::Get, move |req| {
            info!("Received {:?} request for {} configured={}", req.method(), req.uri(), is_online(&ap_mode_clone));
            if is_online(&ap_mode_clone) {
                let mut resp = req.into_ok_response()?;       
                resp.write(b"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 3.2//EN\">
<HTML>
<HEAD>
	<TITLE>Success</TITLE>
</HEAD>
<BODY>
	Success
</BODY>
</HTML>")?;
            } else {let mut resp = req.into_response(302, None, &[("Location", "/config")])?;
                resp.write(b"<HTML><BODY>Not configured</BODY></HTML>")?;
            }
            Ok(())
        })?;

        let ap_mode_clone = ap_mode.clone();
        self.on("/connecttest.txt", Method::Get, move |req| {
            info!("Received {:?} request for {} configured={}", req.method(), req.uri(), is_online(&ap_mode_clone));
            
            if is_online(&ap_mode_clone) {
                let mut resp = req.into_ok_response()?;       
                resp.write(b"Microsoft Connect Test")?;
            } else {
                let mut resp = req.into_response(302, None, &[("Location", "/config")])?;
                resp.write(b"Not configured")?;
            }
            Ok(())
        })?;

        Ok(())
    }
    
}

impl HttpServerManager for EspHttpServerManager<'_> 
{
    fn handle(
        &mut self,
        uri: &str,
        method: HttpMethod,
        f: Box<dyn Fn(&mut dyn Write) -> anyhow::Result<()> + Send>,
    ) -> anyhow::Result<()> {
        
        self.server.fn_handler(uri, to_esp_method(method), move |req| {
            let mut wrapper = WriteWrapper {
                resp: req.into_ok_response()?,
            };

            f(&mut wrapper)
        })?;

        Ok(())
    }

    fn handle_post_form(
        &mut self,
        uri: &str,
        f: Box<
            dyn Fn(&mut dyn Write, IndexMap<String, String>) -> anyhow::Result<()>
                + Send,
        >,
    ) -> anyhow::Result<()> {
        
        self.server.fn_handler(uri, Method::Post, move |mut req| {

            let mut body = Vec::new();
            let mut buf = [0u8; 256];

            loop {
                let read = req.read(&mut buf)?;
                if read == 0 {
                    break;
                }
                body.extend_from_slice(&buf[..read]);
            }

            let form = form_urlencoded::parse(&body)
                .into_owned()
                .collect::<IndexMap<String, String>>();

            let mut wrapper = WriteWrapper {
                resp: req.into_ok_response()?,
            };

            f(&mut wrapper, form)
        })?;

        Ok(())
    }

    fn handle_status(
        &mut self,
        uri: &str,
        method: HttpMethod,
        status: u16,
        message: Option<&'static str>,
        headers: &'static [(&'static str, &'static str)],
        f: Box<dyn Fn(&mut dyn Write) -> anyhow::Result<()> + Send>,
    ) -> anyhow::Result<()> {
        

        self.server.fn_handler(uri, to_esp_method(method), move |req| {

            let resp = req.into_response(
                status,
                message,
                headers,
            )?;

            let mut wrapper = WriteWrapper {
                resp,
            };

            f(&mut wrapper)
        })?;

        Ok(())
    }
}