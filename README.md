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
This crate uses multiple features to support various different ESP32 boards. There are functional features like ```mono-led``` and ```rgb-led``` which are referenced in the code and which get activated by board level features like ```board-cyd``` and ```board-xiao-esp32c6```. Client crates should normally select exactly one board feature and no others.


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
