// Makes ESP-IDF's config visible to the code as cfgs, e.g. esp_idf_esp_https_server_enable, as esp-idf-svc does.
fn main() {
    embuild::espidf::sysenv::output();
}
