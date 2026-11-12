//! Attitude and Heading Reference System
 
use std::{sync::{Arc, RwLock}, time::{Duration, Instant}};
use ahrs::Madgwick;
use ahrs::Ahrs;
use log::info;
use nalgebra::{UnitQuaternion, Vector3};

pub mod qmi8658;

pub const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

pub trait Gyro: Send + Sync {
    fn read_gyro_deg(&self) -> anyhow::Result<Vector3<f32>>;

    fn read_gyro_rad(&self) -> anyhow::Result<Vector3<f32>> {
        Ok(self.read_gyro_deg()? * DEG_TO_RAD)
    }
}


#[derive(Debug, Clone)]
pub struct ImuReading {
        gyroscope: Vector3<f32>,
        accelerometer: Vector3<f32>,
}

#[derive(Debug, Clone)]
pub struct Calibration {
        gyroscope_deg: Vector3<f32>,
        gyroscope_rad: Vector3<f32>,
        accelerometer: UnitQuaternion<f32>,
        magnetometer: Vector3<f32>,
        pitch: f32,
        roll: f32,
}
impl Calibration {
    fn new() -> Self {
        Calibration {
            gyroscope_deg: Vector3::new(0.0, 0.0, 0.0),
            gyroscope_rad: Vector3::new(0.0, 0.0, 0.0),
            accelerometer: UnitQuaternion::identity(),
            magnetometer: Vector3::new(0.0, 0.0, 0.0),
            pitch: 0.0,
            roll: 0.0,
        }
    }
}

pub trait Imu: Gyro {
    fn read_imu_deg(&self) -> anyhow::Result<ImuReading>;

    fn read_imu_rad(&self) -> anyhow::Result<ImuReading> {
        let deg = self.read_imu_deg()?;
        let gyroscope = deg.gyroscope * DEG_TO_RAD;
        
        
        // let g2 = Vector3::new(deg.gyroscope[0].to_radians(),
        //     deg.gyroscope[1].to_radians(), deg.gyroscope[2].to_radians());
        
        // info!("deg={:?} gyro={:?} g2={:?}", deg.gyroscope, gyroscope, g2);
        Ok(ImuReading{
            gyroscope,
            accelerometer: deg.accelerometer,
        })
    }
}

impl<T: Imu> Gyro for T {
    fn read_gyro_deg(&self) -> anyhow::Result<Vector3<f32>> {
        Ok(self.read_imu_deg()?.gyroscope)
    }
}


#[derive(Clone)]
pub struct AhuReading {
        gyroscope: Vector3<f32>,
        accelerometer: Vector3<f32>,
        magnetometer: Vector3<f32>,
}

pub trait Ahu: Imu {
    fn read_ahu_deg(&self) -> anyhow::Result<AhuReading>;

    fn read_ahu_rad(&self) -> anyhow::Result<AhuReading> {
        let deg = self.read_ahu_deg()?;
        Ok(AhuReading{
            gyroscope: deg.gyroscope * DEG_TO_RAD,
            accelerometer: deg.accelerometer,
            magnetometer: deg.magnetometer,
        })
    }
}

impl<T: Ahu> Imu for T {
    fn read_imu_deg(&self) -> anyhow::Result<ImuReading> {
        let ahu = self.read_ahu_deg()?;
        Ok(ImuReading {
            gyroscope: ahu.gyroscope,
            accelerometer: ahu.accelerometer,
        })
    }
}

trait AhrsAnyhowResultExt<T> {
    fn anyhow(self) -> anyhow::Result<T>;
}

impl<T> AhrsAnyhowResultExt<T> for Result<T, ahrs::AhrsError> {
    fn anyhow(self) -> anyhow::Result<T> {
        self.map_err(|e| anyhow::anyhow!("Operation failed: {:?}", e))
    }
}

#[derive(Debug, Clone)]
pub struct Tilt {
    pub roll: f32,
    pub pitch: f32,
    pub raw_roll: f32,
    pub raw_pitch: f32,
}


#[derive(Debug, Clone)]
pub struct Attitude {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct InternalState {
#[cfg(feature = "ahrs")]
    quaternion: UnitQuaternion<f32>,
#[cfg(feature = "tilt")]
    pub raw_pitch: f32,
#[cfg(feature = "tilt")]
    pub raw_roll: f32,
#[cfg(feature = "tilt")]
    pub smoothed_pitch: f32,
#[cfg(feature = "tilt")]
    pub smoothed_roll: f32,
}


// #[cfg(all(feature = "ahrs", not(feature = "tilt")))]
// #[derive(Debug, Copy, Clone)]
// pub struct InternalState {
// #[cfg(feature = "ahrs")]
//     quaternion: UnitQuaternion<f32>,
// }

impl InternalState {
    fn new() -> InternalState {
        InternalState {
#[cfg(feature = "ahrs")]
            quaternion: UnitQuaternion::identity(),
#[cfg(feature = "tilt")]
            raw_pitch: 0.0,
#[cfg(feature = "tilt")]
            raw_roll: 0.0,
#[cfg(feature = "tilt")]
            smoothed_pitch: 0.0,
#[cfg(feature = "tilt")]
            smoothed_roll: 0.0,
        }
    }
}

#[derive(Clone)]
enum Mode {
    Gyro(Arc<dyn Gyro>),
    Imu(Arc<dyn Imu>),
    Ahrs(Arc<dyn Ahu>),
}

pub struct ImuManager
{
    mode:           Mode,
    state:          Arc<RwLock<InternalState>>,
    calibration:    Calibration,
}

impl ImuManager {
    pub fn from_ahrs<T: Ahu + 'static>(device: T) -> ImuManager {
        Self::new(Mode::Ahrs(Arc::new(device)))
    }

    pub fn from_imu<T: Imu + 'static>(device: T) -> ImuManager {
        Self::new(Mode::Imu(Arc::new(device)))
    }

    pub fn from_gyro<T: Gyro + 'static>(device: T) -> ImuManager {
        Self::new(Mode::Gyro(Arc::new(device)))
    }

    fn new(mode: Mode) -> ImuManager {
        let manager = ImuManager {
            mode,
            state: Arc::new(RwLock::new(InternalState::new())),
            calibration: Calibration::new(),
        };
        manager
    }

#[cfg(feature = "ahrs")]
    pub fn read_attitude(&self) -> Attitude {
        let state = *self.state.read().unwrap();

        // Rotate by inverse of reference to get relative orientation
        let relative = self.calibration.accelerometer * state.quaternion;

        let (roll, pitch, yaw) =
                relative.euler_angles();

        Attitude {
            roll: roll.to_degrees(),
            pitch: pitch.to_degrees(),
            yaw: yaw.to_degrees(),
        }
    }

#[cfg(feature = "tilt")]
    pub fn read_tilt(&self) -> Tilt {
        let state = *self.state.read().unwrap();

        Tilt {
            roll: state.smoothed_roll.to_degrees(),
            pitch: state.smoothed_pitch.to_degrees(),
            raw_pitch: state.raw_pitch.to_degrees(),
            raw_roll: state.raw_roll.to_degrees(),
        }
    }

    pub fn start(&mut self, update_frequency: Duration) -> anyhow::Result<()> {
        self.calibrate(update_frequency, 100)?;

        let state = self.state.clone();
        let mode = self.mode.clone();
        let calibration = self.calibration.clone();

        info!("start");

        std::thread::Builder::new()
            .stack_size(8192)
            .name("imu".into())
            .spawn(move || {
                match Self::imu_task(update_frequency, state, mode, calibration) {
                    Ok(_) => log::error!("IMU update task terminated."),
                    Err(error) => log::error!("IMU update task FAILED with {:?}.", error),
                };
            })?;
        
        Ok(())
    }

    pub fn calibrate(&mut self, update_frequency: Duration, samples: usize) -> anyhow::Result<()> {
        info!("Calibrate samples={}", samples);

        let mut calibration = Calibration::new();
        let mut min = Vector3::new(0.0, 0.0, 0.0);
        let mut max = Vector3::new(0.0, 0.0, 0.0);

        for c in 0..samples {
            let now = Instant::now();

            match &self.mode {
                Mode::Gyro(gyro) => {
#[cfg(feature = "ahrs")]
                    let reading_deg = gyro.read_gyro_deg()?;
                    let reading = gyro.read_gyro_rad()?;

                    calibration.gyroscope_deg += reading_deg;
                    calibration.gyroscope_rad += reading;
                },
                Mode::Imu(imu) => {

                        let reading = imu.read_imu_rad()?;
#[cfg(feature = "tilt")]
                    {
                        let raw_pitch = reading.accelerometer.x.atan2((reading.accelerometer.y * reading.accelerometer.y + reading.accelerometer.z * reading.accelerometer.z).sqrt());
                        let raw_roll  = reading.accelerometer.y.atan2(reading.accelerometer.z);
                        calibration.roll += raw_roll;
                        calibration.pitch += raw_pitch;
                    }

#[cfg(feature = "ahrs")]
                    {
                        let reading_deg = imu.read_imu_deg()?;

                        calibration.gyroscope_deg += reading_deg.gyroscope;
                        calibration.gyroscope_rad += reading.gyroscope;

                        if c == 0 || reading_deg.gyroscope.x > max.x {
                            max.x = reading_deg.gyroscope.x;
                        }
                        if c == 0 || reading_deg.gyroscope.y > max.y {
                            max.y = reading_deg.gyroscope.y;
                        }
                        if c == 0 || reading_deg.gyroscope.z > max.z {
                            max.z = reading_deg.gyroscope.z;
                        }
                        if c == 0 || reading_deg.gyroscope.x < min.x {
                            min.x = reading_deg.gyroscope.x;
                        }
                        if c == 0 || reading_deg.gyroscope.y < min.y {
                            min.y = reading_deg.gyroscope.y;
                        }
                        if c == 0 || reading_deg.gyroscope.z < min.z {
                            min.z = reading_deg.gyroscope.z;
                        }
                    }


                    info!("read={:?} calibration={:?}", reading, calibration);
                },
                Mode::Ahrs(ahu) => {
                    let reading = ahu.read_ahu_rad()?;

#[cfg(feature = "tilt")]
                    {
                        let raw_pitch = reading.accelerometer.x.atan2((reading.accelerometer.y * reading.accelerometer.y + reading.accelerometer.z * reading.accelerometer.z).sqrt());
                        let raw_roll  = reading.accelerometer.y.atan2(reading.accelerometer.z);
                        calibration.roll += raw_roll;
                        calibration.pitch += raw_pitch;
                    }

#[cfg(feature = "ahrs")]
                    {
                        let reading_deg = ahu.read_ahu_deg()?;

                        calibration.gyroscope_deg += reading_deg.gyroscope;
                        calibration.gyroscope_rad += reading.gyroscope;
                        calibration.magnetometer += reading_deg.magnetometer;

                        if c == 0 || reading_deg.gyroscope.x > max.x {
                            max.x = reading_deg.gyroscope.x;
                        }
                        if c == 0 || reading_deg.gyroscope.y > max.y {
                            max.y = reading_deg.gyroscope.y;
                        }
                        if c == 0 || reading_deg.gyroscope.z > max.z {
                            max.z = reading_deg.gyroscope.z;
                        }
                        if c == 0 || reading_deg.gyroscope.x < min.x {
                            min.x = reading_deg.gyroscope.x;
                        }
                        if c == 0 || reading_deg.gyroscope.y < min.y {
                            min.y = reading_deg.gyroscope.y;
                        }
                        if c == 0 || reading_deg.gyroscope.z < min.z {
                            min.z = reading_deg.gyroscope.z;
                        }
                    }
                },
            }
            
            

            let elapsed = now.elapsed();

            if elapsed < update_frequency {
                std::thread::sleep(
                    update_frequency - elapsed
                );
            }
        }

        // For the moment I am just averaging the magnetometer but this is not what we really need.
        match &self.mode {
            Mode::Gyro(_gyro) => {
#[cfg(feature = "ahrs")]
                {
                    calibration.gyroscope_deg /= samples as f32;
                    calibration.gyroscope_rad /= samples as f32;
                }
            },
            Mode::Imu(imu) => {
#[cfg(feature = "ahrs")]
                {
                    calibration.gyroscope_deg /= samples as f32;
                    calibration.gyroscope_rad /= samples as f32;
                    
                    Self::calibrate_accelerometer(update_frequency, &mut calibration, &**imu)?;
                }

#[cfg(feature = "tilt")]
                {
                    calibration.roll /= samples as f32;
                    calibration.pitch /= samples as f32;
                }
            },
            Mode::Ahrs(ahu) => {

#[cfg(feature = "ahrs")]
                {
                    calibration.gyroscope_deg /= samples as f32;
                    calibration.gyroscope_rad /= samples as f32;
                    calibration.magnetometer /= samples as f32;
                    
                    Self::calibrate_accelerometer(update_frequency, &mut calibration, &**ahu)?;
                }
#[cfg(feature = "tilt")]
                {
                    calibration.roll /= samples as f32;
                    calibration.pitch /= samples as f32;
                }
            },
        }
        



        info!("Done calibration={:?} min={:?} max={:?}", calibration, min, max);

        self.calibration = calibration;
        Ok(())
    }


#[cfg(feature = "ahrs")]
    fn calibrate_accelerometer(update_frequency: Duration, calibration: &mut Calibration, imu: &dyn Imu) -> anyhow::Result<()> {
        let mut filter = Madgwick::new(update_frequency.as_secs_f32(), 0.1f32);
        let mut last = Instant::now();
        let sample_cnt = 2.0 / update_frequency.as_secs_f32();

        info!("Taking {} samples for madgewick...", sample_cnt);

        for _ in 0..sample_cnt as u32 {
            let now = Instant::now();

            let dt = now.duration_since(last).as_secs_f32();
            *filter.sample_period_mut() = dt;

            last = now;

            let reading = imu.read_imu_rad()?;
            let gyro = reading.gyroscope - calibration.gyroscope_rad;
            filter.update_imu(&gyro, &reading.accelerometer).ok();

            let elapsed = last.elapsed();

            if elapsed < update_frequency {
                std::thread::sleep(
                    update_frequency - elapsed
                );
            }
        }

        calibration.accelerometer = filter.quat().inverse();

        Ok(())
    }

#[cfg(feature = "ahrs")]
    fn adaptive_beta(gyro: &Vector3<f32>, beta_min: f32, beta_max: f32) -> f32 {
        let gyro_norm = gyro.norm();
        let rest_threshold: f32 = 0.04;
        let motion_threshold: f32 = 0.4;
       

        if gyro_norm < rest_threshold {
            beta_min
        } else if gyro_norm > motion_threshold {
            beta_max
        } else {
            let t = (gyro_norm - rest_threshold) / (motion_threshold - rest_threshold);
            beta_min + t * (beta_max - beta_min)
        }
    }

    fn imu_task(
        update_frequency: Duration,
        state: Arc<RwLock<InternalState>>,
        mode: Mode,
        calibration: Calibration,
    ) -> anyhow::Result<()>{
        let alpha: f32 = 0.1;

#[cfg(feature = "ahrs")]
        let mut filter = Madgwick::new(update_frequency.as_secs_f32(), 0.1f32);
        let mut last = Instant::now();
        // let mut cnt = 0;

        let initial_state = *state.read().unwrap();

#[cfg(feature = "tilt")]
        let mut smoothed_pitch = initial_state.smoothed_pitch;
#[cfg(feature = "tilt")]
        let mut smoothed_roll = initial_state.smoothed_roll;


        loop {
            let now = Instant::now();

            let dt =
                now.duration_since(last)
                    .as_secs_f32();

#[cfg(feature = "ahrs")]
            {
                *filter.sample_period_mut() = dt;
            }
            last = now;

            
            // cnt += 1;

            let mut raw_roll = 0.0_f32;
            let mut raw_pitch = 0.0_f32;

#[cfg(feature = "ahrs")]
            let q;
            
            match &mode {
                Mode::Gyro(gyro) => {

#[cfg(feature = "ahrs")]
                    {
                        let mut reading = gyro.read_gyro_rad()?;
                        reading -= calibration.gyroscope_rad;
                        q = filter.update_gyro(
                            &reading,
                        )
                    }
                },
                Mode::Imu(imu) => {
                    let mut reading = imu.read_imu_rad()?;
                    // let raw_reading = reading.clone();

                    // if cnt >9 {
                    //     info!("UPDATE RADS dt={} reading={:?}", 
                    //             dt,
                    //             &reading,
                    //             // &raw_reading,
                    //     );
                    //     cnt = 0;
                    // }

#[cfg(feature = "tilt")]
                    {
                        raw_pitch = reading.accelerometer.x.atan2((reading.accelerometer.y * reading.accelerometer.y + reading.accelerometer.z * reading.accelerometer.z).sqrt());
                        raw_roll  = reading.accelerometer.y.atan2(reading.accelerometer.z);
                    }

#[cfg(feature = "ahrs")]
                    {
                        reading.gyroscope -= calibration.gyroscope_rad;

                        *filter.beta_mut() = Self::adaptive_beta(&reading.gyroscope, 0.033, 0.5);

                        q = filter.update_imu(
                            &reading.gyroscope,
                            &reading.accelerometer
                        ).anyhow()?;
                    }
                },
                Mode::Ahrs(ahu) => {
                    let mut reading = ahu.read_ahu_rad()?;

#[cfg(feature = "tilt")]
                    {
                        raw_pitch = reading.accelerometer.x.atan2((reading.accelerometer.y * reading.accelerometer.y + reading.accelerometer.z * reading.accelerometer.z).sqrt());
                        raw_roll  = reading.accelerometer.y.atan2(reading.accelerometer.z);
                    }

#[cfg(feature = "ahrs")]
                    {
                        
                        reading.gyroscope -= calibration.gyroscope_rad;
                        q = filter.update(
                            &reading.gyroscope,
                            &reading.accelerometer,
                            &reading.magnetometer
                        ).anyhow()?;
                    }
                },
            };

#[cfg(feature = "tilt")]
            {
                raw_pitch -= calibration.pitch;
                raw_roll -= calibration.roll;

                smoothed_pitch = alpha * raw_pitch + (1.0 - alpha) * smoothed_pitch;
                smoothed_roll  = alpha * raw_roll  + (1.0 - alpha) * smoothed_roll;
            }

            let new_state = InternalState {
#[cfg(feature = "ahrs")]
                quaternion: q.clone(),
#[cfg(feature = "tilt")]
                raw_pitch,
#[cfg(feature = "tilt")]
                raw_roll,
#[cfg(feature = "tilt")]
                smoothed_pitch,
#[cfg(feature = "tilt")]
                smoothed_roll,
            };

             

            {
                let mut s =
                    state.write().unwrap();
                *s = new_state;
            }

            let elapsed = last.elapsed();

            if elapsed < update_frequency {
                std::thread::sleep(
                    update_frequency - elapsed
                );
            }
        }
    }
}