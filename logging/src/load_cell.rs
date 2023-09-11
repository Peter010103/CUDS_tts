use rppal::gpio::{Gpio, InputPin, OutputPin};
use std::thread;
use std::time::Duration;

use std::io::{self, BufRead, BufReader, Write};

/// For HX711 interaction via GPIO pins
pub struct Hx711 {
    /// GPIO pin connected to the clock pin of the Hx711
    /// BCM Numbering
    clock_pin: OutputPin,
    /// GPIO pin connected to the data pin of the Hx711
    /// BCM Numbering
    data_pin: InputPin,
}

impl Hx711 {
    pub fn new(clock_pin_id: u8, data_pin_id: u8) -> Hx711 {
        let gpio = Gpio::new().expect("GPIO module failed to initialize");
        let mut clock_pin: OutputPin = gpio
            .get(clock_pin_id)
            .expect("GPIO clock pin invalid")
            .into_output();
        let data_pin: InputPin = gpio
            .get(data_pin_id)
            .expect("GPIO data pin invalid")
            .into_input_pullup();
        clock_pin.set_low();
        Hx711 {
            clock_pin,
            data_pin,
        }
    }

    /// Read from multiple Hx711 devices at once
    ///
    /// Returns a vector of results containing the raw read value
    ///
    /// * 'devices' - The Hx711 device specifications
    ///
    pub fn read_multiple(devices: &mut Vec<Hx711>) -> Vec<Result<f64, &str>> {
        let mut output: Vec<Result<f64, &str>> = Vec::new();
        let mut data: Vec<u32> = Vec::new();
        let mut err: Vec<usize> = Vec::new();

        let mut wait_polls: Vec<u64> = Vec::with_capacity(devices.len());
        for _i in 0..devices.len() as usize {
            wait_polls.push(0);
            data.push(0);
        }
        //Hx711 devices drive their data pins high while they are not ready
        for i in 0..devices.len() as usize {
            while devices.get_mut(i).unwrap().data_pin.is_high() {
                if wait_polls[i] > 1000000 {
                    err.push(i);
                    break;
                }
                wait_polls[i] += 1;
                thread::sleep(Duration::from_micros(0));
            }
        }
        // Pulse 24 times, acquiring data.
        for _i in 0..24 {
            for i in 0..devices.len() as usize {
                devices.get_mut(i).unwrap().clock_pin.set_high();
            }
            for i in 0..devices.len() as usize {
                devices.get_mut(i).unwrap().clock_pin.set_low();
            }

            //Multiple pin read queries can improve error rates
            let mut temp: [u32; 24] = [0; 24];
            for i in 0..devices.len() as usize {
                if err.contains(&i) {
                    continue;
                } else {
                    if devices.get_mut(i).unwrap().data_pin.is_high() {
                        temp[i] = 1;
                    }
                }
            }
            for i in 0..devices.len() as usize {
                if err.contains(&i) {
                    continue;
                } else {
                    if devices.get_mut(i).unwrap().data_pin.is_high() {
                        temp[i] = 1;
                    }
                }
            }
            for i in 0..devices.len() as usize {
                if err.contains(&i) {
                    continue;
                } else {
                    data[i] <<= 1;
                    if devices.get_mut(i).unwrap().data_pin.is_high() || temp[i] > 0 {
                        data[i] += 1;
                    }
                }
            }
        }
        // Pulse 25th time; sets HX711 gain to 128, Ch A
        for i in 0..devices.len() as usize {
            devices.get_mut(i).unwrap().clock_pin.set_high();
        }
        for i in 0..devices.len() as usize {
            devices.get_mut(i).unwrap().clock_pin.set_low();
        }

        for i in 0..devices.len() as usize {
            if err.contains(&i) {
                continue;
            } else {
                if devices.get_mut(i).unwrap().data_pin.is_low() {
                    err.push(i)
                }
            }
        }
        for i in 0..devices.len() as usize {
            if err.contains(&i) {
                output.push(Err("Read Error"))
            } else {
                let f64_data: f64;
                if data[i] & 0x800000 > 0 {
                    //handle two's complement if the input is negative
                    data[i] &= 0x7FFFFF;
                    data[i] ^= 0x7FFFFF;
                    f64_data = ((data[i] + 1) as f64) * -1.0;
                } else {
                    f64_data = data[i] as f64;
                }

                output.push(Ok((f64_data * 1.) / (0x800000 as f64)));
            }
        }
        output
    }

    pub fn read(&mut self) -> Result<i32, &str> {
        self.clock_pin.set_low();
        let mut wait_polls: u64 = 0;
        while self.data_pin.is_high() {
            if wait_polls > 1000000 {
                return Err("HX711 device is not ready");
            }
            wait_polls = wait_polls + 1;
        }
        let mut output: i32 = 0;
        for _i in 0..24 {
            self.clock_pin.set_high();
            output = output << 1;
            self.clock_pin.set_low();
            if self.data_pin.is_high() {
                output = output + 1;
            }
        }
        self.clock_pin.set_high();
        if output & 0x800000 > 0 {
            output = output & 0x7FFFFF;
            output = output ^ 0x7FFFFF;
            output = (output + 1) * -1;
        }
        self.clock_pin.set_low();
        //output = output / 0x800000;

        println!("{:?}", output);
        if self.data_pin.is_low() {
            return Err("HX711 read error");
        }
        Ok(output)
    }
}
