//! The device's identity: its ID, from its MAC address, and the key and self-signed certificate of its HTTPS API, see
//! api.rs. The device makes them the first time it starts and keeps them in NVS, so the certificate's fingerprint
//! stays the same, for an app to pin. An app which flashes the device may put its own in NVS instead, with tls_format
//! set to FORMAT, so that it knows the certificate before the device starts.

use std::ffi::{CStr, CString, c_void};
use std::ptr::null_mut;

use anyhow::{anyhow, bail};
use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use esp_idf_sys::*;
use log::info;

/// The NVS namespace of the device's own settings, which is reserved, see config_store.rs.
pub const DEVICE_NAMESPACE: &str = "device";
const CERT_KEY: &str = "tls_cert";
const KEY_KEY: &str = "tls_key";
/// Which version of generate made the certificate, so that one made by an older version can be made again.
const FORMAT_KEY: &str = "tls_format";
/// 2: with basic constraints, as version 1's had an empty extensions block, which RFC 5280 does not allow, and
/// rustls, and so the app, rejects.
const FORMAT: u8 = 2;
/// Enough for the PEM of an EC P-256 certificate, about 600 bytes, or key, about 230.
const PEM_LEN: usize = 2048;

pub struct DeviceIdentity {
    /// The Wi-Fi MAC address, as 12 lower case hex digits, which never changes.
    pub id: String,
    pub cert_pem: &'static CStr,
    pub key_pem: &'static CStr,
    /// The SHA-256 of the certificate, as lower case hex, for an app to pin.
    pub fingerprint: String,
}

impl DeviceIdentity {
    /// The identity kept in NVS, made and saved first if there is none. Call it once the Wi-Fi is running, as the
    /// random numbers for the key are only truly random then.
    pub fn load_or_create(nvs_partition: &EspNvsPartition<NvsDefault>) -> anyhow::Result<DeviceIdentity> {
        let id = mac_id()?;
        let nvs = EspNvs::new(nvs_partition.clone(), DEVICE_NAMESPACE, true)?;
        let mut cert_buffer = vec![0u8; PEM_LEN];
        let mut key_buffer = vec![0u8; PEM_LEN];
        let current = nvs.get_u8(FORMAT_KEY)? == Some(FORMAT);
        let saved = match (nvs.get_str(CERT_KEY, &mut cert_buffer)?, nvs.get_str(KEY_KEY, &mut key_buffer)?) {
            (Some(cert), Some(key)) if current => Some((cert.to_string(), key.to_string())),
            _ => None,
        };
        let (cert, key) = match saved {
            Some(saved) => saved,
            None => {
                info!("Making the device's HTTPS key and certificate");
                let (cert, key) = generate(&id)?;
                nvs.set_str(CERT_KEY, &cert)?;
                nvs.set_str(KEY_KEY, &key)?;
                nvs.set_u8(FORMAT_KEY, FORMAT)?;
                (cert, key)
            }
        };
        // Kept for as long as the device runs, as the HTTPS server refers to them.
        let cert_pem: &'static CStr = Box::leak(CString::new(cert)?.into_boxed_c_str());
        let key_pem: &'static CStr = Box::leak(CString::new(key)?.into_boxed_c_str());
        let fingerprint = fingerprint(cert_pem)?;
        info!("Device {id}, certificate fingerprint {fingerprint}");
        Ok(DeviceIdentity { id, cert_pem, key_pem, fingerprint })
    }
}

fn mac_id() -> anyhow::Result<String> {
    let mut mac = [0u8; 6];
    check(unsafe { esp_read_mac(mac.as_mut_ptr(), esp_mac_type_t_ESP_MAC_WIFI_STA) }, "esp_read_mac")?;
    Ok(hex(&mac))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn check(result: i32, what: &str) -> anyhow::Result<()> {
    if result < 0 {
        bail!("{what} failed, -0x{:04x}", -result);
    }
    Ok(())
}

/// mbedtls's random number callback, from the hardware random number generator.
extern "C" fn random(_: *mut c_void, output: *mut u8, len: usize) -> i32 {
    unsafe { esp_fill_random(output as *mut c_void, len) };
    0
}

/// A new EC P-256 key, and a self-signed certificate for it, named after the device's ID, valid until 2099, as PEM.
fn generate(id: &str) -> anyhow::Result<(String, String)> {
    unsafe {
        let mut key: mbedtls_pk_context = std::mem::zeroed();
        let mut cert: mbedtls_x509write_cert = std::mem::zeroed();
        mbedtls_pk_init(&mut key);
        mbedtls_x509write_crt_init(&mut cert);
        let result = (|| {
            check(
                mbedtls_pk_setup(&mut key, mbedtls_pk_info_from_type(mbedtls_pk_type_t_MBEDTLS_PK_ECKEY)),
                "mbedtls_pk_setup",
            )?;
            check(
                mbedtls_ecp_gen_key(
                    mbedtls_ecp_group_id_MBEDTLS_ECP_DP_SECP256R1,
                    key.private_pk_ctx as *mut mbedtls_ecp_keypair,
                    Some(random),
                    null_mut(),
                ),
                "mbedtls_ecp_gen_key",
            )?;
            let name = CString::new(format!("CN=sparko-{id}"))?;
            mbedtls_x509write_crt_set_version(&mut cert, MBEDTLS_X509_CRT_VERSION_3 as i32);
            mbedtls_x509write_crt_set_md_alg(&mut cert, mbedtls_md_type_t_MBEDTLS_MD_SHA256);
            mbedtls_x509write_crt_set_subject_key(&mut cert, &mut key);
            mbedtls_x509write_crt_set_issuer_key(&mut cert, &mut key);
            check(mbedtls_x509write_crt_set_subject_name(&mut cert, name.as_ptr()), "set_subject_name")?;
            check(mbedtls_x509write_crt_set_issuer_name(&mut cert, name.as_ptr()), "set_issuer_name")?;
            // A random positive serial number.
            let mut serial = [0u8; 16];
            esp_fill_random(serial.as_mut_ptr() as *mut c_void, serial.len());
            serial[0] = (serial[0] & 0x7f) | 0x01;
            check(mbedtls_x509write_crt_set_serial_raw(&mut cert, serial.as_mut_ptr(), serial.len()), "set_serial")?;
            check(
                mbedtls_x509write_crt_set_validity(&mut cert, c"20250101000000".as_ptr(), c"20991231235959".as_ptr()),
                "set_validity",
            )?;
            // Not a CA. Also so that there is an extension, as an empty extensions block is not allowed, see FORMAT.
            check(mbedtls_x509write_crt_set_basic_constraints(&mut cert, 0, -1), "set_basic_constraints")?;

            let mut buffer = vec![0u8; PEM_LEN];
            check(
                mbedtls_x509write_crt_pem(&mut cert, buffer.as_mut_ptr(), buffer.len(), Some(random), null_mut()),
                "mbedtls_x509write_crt_pem",
            )?;
            let cert_pem = CStr::from_ptr(buffer.as_ptr() as *const _).to_str()?.to_string();
            buffer.fill(0);
            check(mbedtls_pk_write_key_pem(&mut key, buffer.as_mut_ptr(), buffer.len()), "mbedtls_pk_write_key_pem")?;
            let key_pem = CStr::from_ptr(buffer.as_ptr() as *const _).to_str()?.to_string();
            buffer.fill(0);
            Ok((cert_pem, key_pem))
        })();
        mbedtls_x509write_crt_free(&mut cert);
        mbedtls_pk_free(&mut key);
        result
    }
}

/// The SHA-256 of the certificate's DER, as lower case hex, as e.g. openssl x509 -fingerprint -sha256 shows it, less
/// the colons.
fn fingerprint(cert_pem: &CStr) -> anyhow::Result<String> {
    unsafe {
        let mut cert: mbedtls_x509_crt = std::mem::zeroed();
        mbedtls_x509_crt_init(&mut cert);
        let bytes = cert_pem.to_bytes_with_nul();
        let result = check(mbedtls_x509_crt_parse(&mut cert, bytes.as_ptr(), bytes.len()), "mbedtls_x509_crt_parse")
            .and_then(|_| {
                let mut hash = [0u8; 32];
                check(mbedtls_sha256(cert.raw.p, cert.raw.len, hash.as_mut_ptr(), 0), "mbedtls_sha256")?;
                Ok(hex(&hash))
            });
        mbedtls_x509_crt_free(&mut cert);
        result.map_err(|error| anyhow!("the device's certificate cannot be read, {error:#}"))
    }
}
