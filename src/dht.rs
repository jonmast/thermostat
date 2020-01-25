use rppal::gpio::{IoPin, Level, Mode};
use std::error::Error;
use std::fmt::Display;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use libc::{sched_param, sched_setscheduler, SCHED_FIFO, SCHED_OTHER};
const MAX_COUNT: usize = 32000;
const DHT_PULSES: usize = 41;

pub struct Reading {
    pub humidity: f32,
    pub temperature: f32,
}

pub fn read(pin: &mut IoPin) -> Result<Reading, ReadingError> {
    let mut pulse_counts: [usize; DHT_PULSES * 2] = [0; DHT_PULSES * 2];

    // Bump up process priority and change scheduler to try to try to make process more 'real time'.
    set_max_priority();

    pin.set_mode(Mode::Output);
    pin.write(Level::High);
    sleep(Duration::from_millis(1));

    // The next calls are timing critical and care should be taken
    // to ensure no unnecssary work is done below.

    pin.write(Level::Low);
    sleep(Duration::from_micros(1100));

    pin.set_mode(Mode::Input);

    // Need a very short delay before reading pins or else value is sometimes still low.
    //
    // Note: doesn't seem to really help in practice, maybe because we're not realtime enough
    delay_microseconds(5);

    // Wait for sensor to pull pin low.
    let mut count = 0;
    while pin.read() == Level::High {
        count += 1;

        if count > MAX_COUNT {
            return Err(ReadingError::Timeout);
        }
    }

    for c in 0..DHT_PULSES {
        let i = c * 2;

        while pin.read() == Level::Low {
            pulse_counts[i] += 1;

            if pulse_counts[i] > MAX_COUNT {
                return Result::Err(ReadingError::Timeout);
            }
        }

        while pin.read() == Level::High {
            pulse_counts[i + 1] += 1;

            if pulse_counts[i + 1] > MAX_COUNT {
                return Result::Err(ReadingError::Timeout);
            }
        }
    }

    set_default_priority();

    decode(pulse_counts)
}

fn decode(arr: [usize; DHT_PULSES * 2]) -> Result<Reading, ReadingError> {
    let mut threshold: usize = 0;

    let mut i = 2;
    while i < DHT_PULSES * 2 {
        threshold += arr[i];

        i += 2;
    }

    threshold /= DHT_PULSES - 1;

    let mut data = [0 as u8; 5];
    let mut i = 3;
    while i < DHT_PULSES * 2 {
        let index = (i - 3) / 16;
        data[index] <<= 1;
        if arr[i] >= threshold {
            data[index] |= 1;
        } else {
            // else zero bit for short pulse
        }

        i += 2;
    }

    if data[4]
        != (data[0]
            .wrapping_add(data[1])
            .wrapping_add(data[2])
            .wrapping_add(data[3])
            & 0xFF)
    {
        return Result::Err(ReadingError::Checksum);
    }

    let h_dec = data[0] as u16 * 256 + data[1] as u16;
    let h = h_dec as f32 / 10.0f32;

    let t_dec = (data[2] & 0x7f) as u16 * 256 + data[3] as u16;
    let mut t = t_dec as f32 / 10.0f32;
    if (data[2] & 0x80) != 0 {
        t *= -1.0f32;
    }

    Result::Ok(Reading {
        temperature: t,
        humidity: h,
    })
}

fn set_max_priority() {
    unsafe {
        let param = sched_param { sched_priority: 32 };
        let result = sched_setscheduler(0, SCHED_FIFO, &param);

        if result != 0 {
            panic!("Error setting priority, you may not have cap_sys_nice capability");
        }
    }
}

fn set_default_priority() {
    unsafe {
        let param = sched_param { sched_priority: 0 };
        let result = sched_setscheduler(0, SCHED_OTHER, &param);

        if result != 0 {
            panic!("Error setting priority, you may not have cap_sys_nice capability");
        }
    }
}

// Adapted from WiringPi
// https://github.com/WiringPi/WiringPi/blob/093e0a17a40e064260c1f3233b1ccdf7e4c66690/wiringPi/wiringPi.c#L2114
#[inline(always)]
fn delay_microseconds(micros: u64) {
    if micros == 0 {
        // Zero sleep is a no-op
    } else if micros < 100 {
        delay_microseconds_hard(micros);
    } else {
        sleep(Duration::from_micros(micros));
    }
}

#[inline(always)]
fn delay_microseconds_hard(micros: u64) {
    let time = SystemTime::now();

    loop {
        match time.elapsed() {
            Ok(duration) => {
                if duration >= Duration::from_micros(micros) {
                    return;
                }
            }
            // System clock has gone backwards, just abort
            Err(_) => return,
        }
    }
}

#[derive(Debug)]
pub enum ReadingError {
    Timeout,
    Checksum,
}

impl Display for ReadingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Err: {:?}", self)
    }
}

impl Error for ReadingError {}
