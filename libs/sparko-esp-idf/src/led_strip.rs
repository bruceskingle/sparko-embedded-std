use esp_idf_sys::*;
use core::ptr;

pub struct LedStrip {
    strip: *mut led_strip_handle_t,
}

impl LedStrip {
    pub fn new() -> Self {
    unsafe {
        let strip_config = led_strip_config_t {
            strip_gpio_num: 18,
            max_leds: 64,
            led_pixel_format: led_pixel_format_t_LED_PIXEL_FORMAT_GRB,
            led_model: led_model_t_LED_MODEL_WS2812,
            flags: led_strip_config_t__bindgen_ty_1 {
                invert_out: 0,
            },
        };

        let rmt_config = led_strip_rmt_config_t {
            clk_src: rmt_clock_source_t_RMT_CLK_SRC_DEFAULT,
            resolution_hz: 10_000_000,
            mem_block_symbols: 64,
            flags: led_strip_rmt_config_t__bindgen_ty_1 {
                with_dma: 1, // ignored on ESP32, used on S3/C3
            },
        };

        let mut strip: *mut led_strip_handle_t = ptr::null_mut();

        led_strip_new_rmt_device(&strip_config, &rmt_config, &mut strip);
    }
        Self { strip }
    }
}