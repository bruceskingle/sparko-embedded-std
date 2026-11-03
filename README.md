# sparko-embedded-std
Sparko Embedded Std is a platform for embedded applications for hardware such as ESP32 SoC boards. This platform includes the standard Rust library which means that the heap and standard collections like ```Vec``` are all available for use. The platform currently supports, and was designed for, ESP32 boards and uses the ```esp-idf-hal``` and ```esp-idf-svc``` crates. It is possible that this platform will support other hardware architectures in the future (such as Raspberry PI boards) but for the time being it is ESP32 only.

# The Core Platform
The core platform provides a set of services to support the running of one or more *Features*, which provide the application functionality. Each feature can be enabled or disabled and may have a configuration. The *Core Feature* provides WiFi and an HTTP server. If the *Core Feature* does not have a valid configuration (including the SSID and password for your WiFi network), then it will start WiFi an *Access Point Mode*. This means that it will create a new WiFi network (with the SSID **ESP32-Setup** and the password **password**) with a captive portal so that if you connect to this network you will be taken automatically to the config page so the necessary configuration values can be entered. Once this is done and the board is rebooted it will start WiFi in client mode, connecting to the WiFi network you have configured as a normal device.

The HTTP server is still available (on the IP address allocated by your WiFi network) which means that further configuration is possible and this HTTP server can also be used by *Features* to provide application functionality. The *Core Feature* includes an mDNS server so that you can easily find the board using a DNS name in the **.local** domain, and a task scheduler which can be used to run task processes on a predetermined schedule using [Cron](https://en.wikipedia.org/wiki/Cron) like schedules, and an [NTP](https://en.wikipedia.org/wiki/Ntpd) client which synchronies the system clock with real time.

# A Simple Example Application
This is the sample application for the ESP32 DevKitV1 board:

```rust
use sparko_esp_idf::{
    Esp32Platform,
    features::{binary_clock::BinaryClock, dyndns2::DynDns2},
};

fn main() -> anyhow::Result<()> {
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = sparko_esp_idf::smart_led::new(
        remainder.spi3,
        remainder.gpio14, //SCLK
        remainder.gpio13, //SDO / MISO
        64,
    )?;

    let platform = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(BinaryClock::new_spi(smart_leds)))?
        .build()?;

    platform.start()
}
```
The ```Esp32Platform::builder()``` method returns  a builder for the Platform, which can be used to add *Features* to the Platform, and a ```Remainder``` struct which contains all the hardware resources provided by the board which are not used by the Platform itself. This example includes two *Features* which are provided by the platform. ```DynDns2``` is a Dynamic DNS updater which periodically checks the device's public IP address and when it changes updates a DNS entry. ```BinaryClock``` is a *Feature* which uses WS2812 Smart LEDs to display the date and time in binary.

Two implementations of the Smart LED driver are provided, the preferred approach is to use an RMT device to manage the timings for the Smart LED protocol, but the RMT devices on older ESP32 chips (including the DevKitV1 board) are unreliable, and an alternative SPI based driver is used in those cases. The ```smart_leds``` driver is given an ```SPI``` device and two ```GPIO``` pins from the ```Remainder``` struct, and this is passed to ```BinaryClock``` *Feature*. Finally the platform is created by calling ```.build()``` on the builder, and the ```start()``` method is called, which should never return.

# Why ```std``` and why ```esp-idf-svc```
There are other approaches based on bare metal (non std) architecture (e.g. using ```esp-hal```) and there are lots of great resources to help you to get started with that approach. I started out using this approach and I found the videos on the Youtube ```Rusty Bits``` channel (https://www.youtube.com/watch?v=dxgufYRcNDg) really helpful, so if you would prefer a bare metal approach that would be a great place to start. I was able to get examples to blink an LED, drive WS2812 Smart Leds and draw on the screen of a "Cheap Yellow Display" following those videos. The next thing I wanted to do was to get WiFi working to get NTP time synchronization but although there is code to support WiFi on EP32 in the ```esp-hal``` ecosystem, I was unable to get it to work reliably.

In a search for an alternative approach I came across the YouTube ```Floodplain``` channel (https://www.youtube.com/watch?v=o8yNNVFzNnM&t=9s) and based on that approach, and using the ```esp-idf-svc``` crate I was able to get WiFi working successfully. Binaries build on the standard library and using ```esp-idf-svc``` are larger than bare metal binaries, but I have not had any trouble running them on any ESP32 based board including the DevKitV1, and while this is not a pure Rust implementation (the ```esp-idf-``` crates are a thin Rust wrapper for the C based ESP IDF SDK) those underlying C implementations (especially the WiFi) are production tested mature implementations which I have found to work well.

# Architecture
This repo contains a number of library crates under the ```libs``` directory and a set of example binary crates under the ```examples``` directory:
```
├── examples
│   ├── cyd
│   ├── devkitv1
│   ├── supermini-esp32c3
│   ├── wave-esp32c6147
│   ├── wave-esp32c6touch147
│   ├── waveshare-s3-rlcd42
│   └── xiao-esp32c6
└── libs
    ├── sparko-embedded-std
    │   └── examples
    └── sparko-esp-idf

```
## sparko-embedded-std
This crate contains pure rust code (without any ESP32 specific dependencies). This separation may become useful in the future when other hardware families are supported but for the time being is somewhat academic.

## sparko-esp-idf
This crate contains all platform code which is specific to the ESP-IDF based EAP32 implementation (as well as some code which arguably should be in ```sparko-embedded-std```). Support for various features of the platform is enabled via ```Cargo``` features

# Design Patterns
The platform makes use of the builder pattern as a way of trying to get the best trade off between the cost and benefits of the standard library. Most of the platform types have a builder containing collections which allow items to be added, but the ```build()``` method will usually call ```.shrink_to_fit()``` on those collections which are then treated as immutable from that point onwards.

The conventions for builders are that they
- are constructed by calling the ```builder()``` associated function on the class being constructed, e.g. ```TaskManager::builder()```
- provide chainable methods with names starting ```with_``` e.g. 
```
    TaskManager::builder()
        .with_task(task1)?
        .with_task(task2)?
        .build();
```
- provide callable methods with names starting ```add_``` e.g.
```
    let mut builder = TaskManager::builder()
        .with_task(task1)?;
    
    builder.add_task(task2)?;
    
    builder.build();
```
- have methods which either return their result directly (if there are no failure scenarios) or return an ```anyhow::Result<>``` of their result.

The following implementations are available which use this crate:
- [sparko-esp-idf](https://github.com/bruceskingle/sparko-esp-idf) for ESP32 SoC based boards.

Example applications for various boards are available at [sparko-embedded-examples](https://github.com/bruceskingle/sparko-embedded-example) on GitHub.

# Features
This crate uses multiple features to support various different ESP32 boards. There are functional features like ```mono-led``` and ```rgb-led``` which are referenced in the code and which get activated by board level features like ```board-cyd``` and ```board-xiao-esp32c6```. Client crates should normally select exactly one board feature, and any of the optional hardware features (see below) which their board has and which they want to use.

# Board Capabilities
The platform drives the hardware a board always has itself, such as its display or status LED, and offers the hardware a project may or may not want, because its pins could be used for something else, as optional ```Cargo``` features. Setting one of these features gives that hardware's pins to the platform, so they are no longer in the ```Remainder```, and setting it for a board which does not have that hardware is a compile error.

Two different things are called features here. A ```Cargo``` feature, such as ```key-button```, decides at compile time who owns some hardware. A *Feature*, such as ```AnalogClock```, is a piece of application functionality which can be enabled or disabled on the device's config page at run time.

## Push Buttons
A board's buttons are owned by the platform with features such as ```key-button``` and ```boot-button```. The platform watches each button from the start, and a press runs the button's *action*, if it has one, on the button's own thread, so an action may take its time, for example to show a message for a second. Presses while an action is running are not counted.

The application gives a button its action with the builder. The closure is called with the platform once it is running, before the *Features* start, so that it can take what the action needs, such as the display manager:
```rust
let platform = builder
    .with_key_button(|sparko| {
        let display_manager = sparko.display_manager.clone();
        Ok(Box::new(move || message::show(&display_manager, "KEY pressed")))
    })
    .build()?;
```
A *Feature* can take a button over when it starts, replacing the application's action, as it is given the platform:
```rust
fn start(&mut self, sparko: &mut Esp32Platform, initializer: &mut Esp32PlatformInitializer, config: Self::Config) -> anyhow::Result<()> {
    let display_manager = sparko.display_manager.clone();
    sparko.key_button().set_action(Box::new(move || next_page(&display_manager)));
    Ok(())
}
```
```clear_action()``` makes a button do nothing again. A button wired to a pin of the ```Remainder``` works in the same way, with ```PushButton::new("Name", remainder.gpio4)?``` and then ```set_action()```. Buttons are taken to connect their pin to ground when pressed, using the chip's own pull-up, as board buttons usually do.

## SD Cards
With the ```sd-card``` feature the platform mounts the board's SD card at start-up, with FAT, at ```/sd```, so that it can be used with ```std::fs```. ```sparko.sd_card()``` gives the mount point, or why there is no card, and ```sd_card::space()``` its size and free space. The card is only looked for at start-up, so a card put in later is found when the board next restarts.

## Displays
On a board with a display, ```sparko.display_manager``` is the board's ```DisplayManager```, which *Features* draw with, using ```embedded-graphics```. On a black and white display (the ```mono-display``` feature) colours are drawn as ink or paper by their brightness, so that a design for a colour display, light on dark, comes out dark on light, and *Features* offer no colour settings.

## Temperature and Humidity
On a board with a temperature and humidity sensor, such as the SHTC3, ```sparko.shtc3().read()``` gives a reading. The platform adds a *Feature* of its own, **Temperature and Humidity**, which logs a reading every minute and has a **Temperature Offset** setting, in tenths of a degree. A sensor on the board is warmed by it, so it reads high: set the offset by comparing with a thermometer which can be trusted, after the board has been running for a while. The humidity is corrected with it.

## Speaker
On a board with a speaker, ```sparko.speaker()``` plays sounds: ```play_tone(880.0, Duration::from_millis(120))``` for a beep, or ```play_wav(bytes)``` for a WAV file of 16-bit PCM, of any sample rate, mono or stereo, which is mixed to mono and resampled to the speaker's 16 kHz. A sound can be built into the firmware with ```include_bytes!("../sounds/chime.wav")```, or read from the SD card. Each play waits until the sound has finished, so play from a thread of its own to carry on meanwhile. The platform adds a *Feature* of its own, **Speaker**, whose **Volume (%)** setting, a slider which plays a tone at each volume it is moved to, is what every sound plays at, from as soon as the board starts; 100% is the codec's full level, and each percent less is half a dB quieter.

## Sliders
A number setting can be shown on the config page as a slider, with ```#[config(min = 0, max = 100)]``` on an ```i32``` or ```i64``` field of a *Feature*'s config. ```default = N``` is where the slider is while the setting has not been saved, and ```preview = "/path"``` a path on the device which the page POSTs each position to, as ```value=N```, while the slider is moved, so that its effect can be tried before it is saved, as the speaker's volume is. A position is sent only once the last has been answered, so that a fast drag does not queue up requests.

## Chip and Board
The config page, and ```/api/info``` where the API is enabled, say which board the firmware was built for and the chip it is running on, read from the chip itself: its model, revision, cores, flash and PSRAM. The board cannot be read from the hardware, only the chip.


# Development
In order to avoid compiler errors during development one of the board features should be enabled in VSCode settings (file ```.vscode/settings.json``` in the workspace root) and when building on the command line release mode and one board feature should be selected e.g. ```cargo build --release --features board-cyd```

# Supported Boards
The following boards are currently supported:

## board-cyd
The so called "Cheap Yellow Display" or more properly the **ESP32-2432S028R** is a 
development board has become known in the maker community as the “Cheap Yellow Display” or CYD for short. This development board, whose main chip is an ESP32-WROOM-32 module, comes with

- 2.8-inch TFT touchscreen LCD
- microSD card interface
- RGB LED
- built-in LDR (light-dependent resistor)      d
- all the required circuitry to program and apply power to the board.

Useful board information can be found at [Random Nerd Tutorials](https://randomnerdtutorials.com/?s=CYD)
I have read that there are clones of this board which have slight differences, the one I developed on cam from [Ali Express](https://www.aliexpress.com/item/1005008229897039.html)

## board-xiao-esp32c6
This is the [Seed Studio XIAO ESP32-C6](https://www.seeedstudio.com/Seeed-Studio-XIAO-ESP32C6-p-5884.html) board which combines 2.4GHz Wi-Fi 6 (802.11ax), Bluetooth 5(LE), and IEEE 802.15.4 radio connectivity with a C6 processor.

## board-waveshare-s3-rlcd42
This is the [Waveshare ESP32-S3-RLCD-4.2](https://www.waveshare.com/product/esp32-related/boards-kits/esp32-s3/esp32-s3-rlcd-4.2.htm), whose ESP32-S3-WROOM-1-N16R8 module has two cores, 16 MB of flash and 8 MB of PSRAM, which the platform adds to the heap. It comes with

- a 4.2-inch reflective LCD, 400 x 300, black and white, with an ST7305 controller, and no backlight: it is lit by the room, like e-paper, and readable in daylight. The outermost row and column of pixels on each edge are hidden by its frame, so the platform offers the 398 x 298 which can be seen. The example's **Edge Test** *Feature* draws a pattern to check this.
- a microSD card slot, wired for SDMMC with one data line (the ```sd-card``` feature)
- a KEY button (the ```key-button``` feature), and a BOOT button which, once the board has started, can be used as a button too (the ```boot-button``` feature)
- an SHTC3 temperature and humidity sensor, which the platform always drives
- an ES8311 audio codec and amplifier, with a connector for the speaker which comes with it, which the platform always drives
- a PCF85063 real time clock, and two microphones, which the platform does not drive yet
- a 2 x 8 pin, 2.54 mm header, with 3V3, VBUS, two grounds, and GPIO 0, 1, 2, 3, 17, 18, 19, 20, 43, 44, 13 and 14. GPIO 1, 2, 3 and 17 are free, in the ```Remainder```. 13 and 14 are the board's I2C bus, which more devices can join, using ```sparko.i2c()```. 0 and 18 are the buttons, 19 and 20 the USB port, and 43 and 44 the console's UART, which the platform keeps
- a holder for an 18650 lithium battery, with charging, and a PWR button which switches the battery's power. The battery is not supplied: an unprotected 18650, flat or button top, about 65 mm long, fits.

There is no reset button: unplug it, use the config page's **Restart** command, or reset it over USB. The example application, in ```boards/waveshare-s3-rlcd42```, plays ```sounds/Startup.wav``` as it starts, and shows an analog clock, the SD card's size and free space when KEY is pressed, and the temperature and humidity when BOOT is pressed, each with a beep.
