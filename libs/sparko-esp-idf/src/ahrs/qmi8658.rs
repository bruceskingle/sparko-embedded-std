use std::{sync::{Arc, Mutex}, time::Duration};

use nalgebra::Vector3;

use crate::ahrs::{Imu, ImuReading};


const QMI8658_ADDR: u8 = 0x6B;

// const REG_WHO_AM_I: u8 = 0x00;

const REG_CTRL1: u8 = 0x02;     // CTRL1 — system + mode control
const REG_CTRL2: u8 = 0x03;
const REG_CTRL3: u8 = 0x04;
const REG_CTRL5: u8 = 0x06;
const REG_CTRL7: u8 = 0x08;

const REG_AX_L: u8 = 0x35;

const I2C_TIMEOUT_US: u32 = 1000;

pub struct Qmi8658 {
    i2c: Arc<Mutex<esp_idf_hal::i2c::I2cDriver<'static>>>,
    acc_scale: f32,
    gyro_scale: f32,
}

impl Qmi8658 {
    pub fn new(i2c: &Arc<Mutex<esp_idf_hal::i2c::I2cDriver<'static>>>,) -> anyhow::Result<Self> {
        let manager = Self {
            i2c: i2c.clone(),
            acc_scale: 4096.0,  // ±8g
            gyro_scale: 128.0,   // ±256 dps
        };

        manager.init()?;
         
        Ok(manager)
    }

    fn init(&self) -> anyhow::Result<()> {

        let mut i2c = self.i2c.lock().unwrap();
        //
        // sensor enabled
        // auto increment enabled (important for burst reads)
        //
        i2c.write(
            QMI8658_ADDR,
            &[REG_CTRL1, 0x60],
            I2C_TIMEOUT_US
        )?;

        //
        // accel:
        //   ±8g
        //   1000Hz ODR
        //
        i2c.write(
            QMI8658_ADDR,
            &[REG_CTRL2, 0x23],
            I2C_TIMEOUT_US
        )?;

        //
        // gyro:
        //   256dps
        //   1000Hz ODR
        //
        i2c.write(
            QMI8658_ADDR,
            &[REG_CTRL3, 0x43],
            I2C_TIMEOUT_US
        )?;


        // CTRL5 — filter (optional but important)
        // 0x01 or 0x00
        // Waveshare examples often leave it default.
        i2c.write(
            QMI8658_ADDR,
            &[REG_CTRL5, 0x01],
            I2C_TIMEOUT_US
        )?;

        //
        // enable accel + gyro
        //
        i2c.write(
            QMI8658_ADDR,
            &[REG_CTRL7, 0x03],
            I2C_TIMEOUT_US
        )?;

        std::thread::sleep(Duration::from_millis(100));

        Ok(())
       
    }
}

impl Imu for Qmi8658 {
    fn read_imu_deg(&self) -> anyhow::Result<super::ImuReading> {
        let mut buf = [0u8; 12];

        //
        // Read:
        // accel xyz + gyro xyz
        //
        self.i2c.lock().unwrap().write_read(
            QMI8658_ADDR,
            &[REG_AX_L],
            &mut buf,
            I2C_TIMEOUT_US
        )?;

        let ax = i16::from_le_bytes([buf[0], buf[1]]);
        let ay = i16::from_le_bytes([buf[2], buf[3]]);
        let az = i16::from_le_bytes([buf[4], buf[5]]);

        let gx = i16::from_le_bytes([buf[6], buf[7]]);
        let gy = i16::from_le_bytes([buf[8], buf[9]]);
        let gz = i16::from_le_bytes([buf[10], buf[11]]);

        //
        // Convert to engineering units
        //

        // ±8g
        let ax_g = ax as f32 / self.acc_scale;
        let ay_g = ay as f32 / self.acc_scale;
        let az_g = -az as f32 / self.acc_scale;

        // ±512 dps
        let gx_dps = gx as f32 / self.gyro_scale;
        let gy_dps = gy as f32 / self.gyro_scale;
        let gz_dps = -gz as f32 / self.gyro_scale;

        Ok(ImuReading{
            accelerometer: Vector3::new(ax_g, ay_g, az_g),
            gyroscope: Vector3::new(gx_dps, gy_dps, gz_dps),
        })
    }
}