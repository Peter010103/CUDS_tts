use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod load_cell;

use serialport::SerialPort;
use std::io::BufRead;
use std::io::BufReader;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use csv::WriterBuilder;
use std::fs::OpenOptions;

use std::env;

// Max PWM
const MAX_PWM: i32 = 1600;

// Calculation Constants
const MOTOR_MAGNET_POLES: f64 = 14.0;
const CALIBRATION_GRADIENTS: [f64; 1] = [2.46324555e-5];

fn main() {
    // Initialise serial port
    let port = serialport::new("/dev/ttyACM0", 115_200)
        .timeout(Duration::from_millis(6000))
        .open()
        .expect("Failed to open serial port");

    // Wrap the port in a mutex
    let port = Arc::new(Mutex::new(port));
    let port_clone = port.clone();

    // Create Load Cell devices
    let mut load_cell_devices: Vec<load_cell::Hx711> = Vec::new();
    let mut config_load_cell_pins_list: Vec<(u8, u8)> = Vec::new();

    // Note BCN pin numbering (i.e. GPIO_XX)
    config_load_cell_pins_list.push((7, 1));
    //config_load_cell_pins_list.push((20, 21));
    //config_load_cell_pins_list.push((25, 8));

    for load_cell_pins in &config_load_cell_pins_list {
        let hx711 = load_cell::Hx711::new(load_cell_pins.0, load_cell_pins.1);
        load_cell_devices.push(hx711);
    }

    // Args for filename
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <filename.csv>", args[0]);
        return;
    }

    let filename = &args[1];
    if !filename.ends_with(".csv") {
        println!("The provided filename does not end with \".csv\"");
        return;
    }

    println!("Saving results to {}", filename);

    // CSV file output
    let file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open(filename)
        .expect("Failed to create data file");

    let mut csv_writer = WriterBuilder::new()
        .has_headers(true)
        .delimiter(b',')
        .from_writer(file);

    // Manually write the header record
    csv_writer
        .write_record(&["Timestamp", "DShot_cmd", "Thrust", "Voltage", "Omega"])
        .expect("Failed to create headers for csv");

    // Set running flag
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Create Ctrl+C handler
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C signal");
        running_clone.store(false, Ordering::SeqCst);
        exit_sequence(&port_clone);
    })
    .expect("Error setting Ctrl+C handler");

    println!("Calibrating load cells");
    let offset: Vec<f64> = calibrate_zero(&mut load_cell_devices);

    initialise(&port);

    let mut throttle_command: i32 = 0;
    while running.load(Ordering::SeqCst) && throttle_command < MAX_PWM {
        throttle_command = throttle_command + 50;
        println!("Sent throttle: {}", throttle_command);

        let telem_data = control_sequence(&port);
        let mut thrust = 0.0;

        let load_cell_read = load_cell::Hx711::read_multiple(&mut load_cell_devices);
        for i in 0..load_cell_read.len() {
            if let Ok(read_result) = load_cell_read
                .get(i)
                .expect("Tried to read from non-existent load cell")
            {
                thrust = (1.0 / CALIBRATION_GRADIENTS[i]) * read_result + offset[i];
                println!("Measured Thrust: {:.2}", thrust);
                // print!("Load cell {}: {:.8} \t", i, read_result);
            }
        }

        let mut sample = vec![
            throttle_command.to_string(),
            format!("{:3}", thrust.to_string()),
        ];
        sample.extend(telem_data);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Could not retrieve time");
        sample.insert(0, timestamp.as_secs_f64().to_string());

        // Timestamp, Throttle command, Thrust, Voltage, Omega
        let _ = csv_writer.write_record(&sample);

        println!("{:.2}", timestamp.as_secs_f64());
        println!("");
    }

    let _ = csv_writer.flush();
    thread::sleep(Duration::from_millis(100));
    exit_sequence(&port);
}

fn process_string(data: &str) -> Vec<String> {
    let string = data.trim_end_matches("\r\n");

    let telemetry: Vec<u8> = string
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let hex_str = std::str::from_utf8(chunk).expect("Invalid UTF-8 chunk");
            u8::from_str_radix(hex_str, 16).expect("Failed to parse hexadecimal")
        })
        .collect();

    let temperature = telemetry[0];
    let voltage = (((u16::from(telemetry[1])) << 8) | u16::from(telemetry[2])) as f64 / 100.0;
    let current = (((u16::from(telemetry[3])) << 8) | u16::from(telemetry[4])) as f64 / 100.0;
    let m_ah = (((u16::from(telemetry[5])) << 8) | u16::from(telemetry[6])) as f64;
    let erpm = (100 * ((u32::from(u16::from(telemetry[7])) << 8) | u32::from(telemetry[8]))) as f64;

    let omega = erpm * 2.0 * 3.14 / 60.0 / MOTOR_MAGNET_POLES / 2.0;

    // println!("{:?}", telemetry);
    println!("Temperature: {}", temperature);
    println!("Voltage: {:.2}", voltage);
    println!("Current: {:.2}", current);
    println!("Consumption: {}", m_ah);
    println!("Omega: {:2}", omega);

    let row = vec![voltage.to_string(), omega.to_string()];
    row
}

fn initialise(port: &Arc<Mutex<Box<dyn SerialPort>>>) {
    let mut locked_port = port.lock().unwrap();

    // Initialise dshot: Send arming and onewire command
    locked_port
        .write(" ".as_bytes())
        .expect("Write to serial port failed");
    thread::sleep(Duration::from_millis(100));
    locked_port
        .write("t".as_bytes())
        .expect("Write to serial port failed");
    thread::sleep(Duration::from_millis(100));
}

fn exit_sequence(port: &Arc<Mutex<Box<dyn SerialPort>>>) {
    let mut locked_port = port.lock().unwrap();

    // Send zero throttle command
    locked_port
        .write(" \n".as_bytes())
        .expect("Write to serial port failed");
    let _ = locked_port.flush();

    std::thread::sleep(Duration::from_millis(100));
    println!("Closing port");
    std::mem::drop(locked_port)
}

fn control_sequence(port: &Arc<Mutex<Box<dyn SerialPort>>>) -> Vec<String> {
    let mut locked_port = port.lock().unwrap();

    // Increment throttle by 50
    locked_port
        .write("r".as_bytes())
        .expect("Write to serial port failed");

    // Wait for telemetry values to update
    thread::sleep(Duration::from_millis(3000));

    let mut reader = BufReader::new(&mut *locked_port);
    let mut telemetry = String::new(); // Use a Vec<u8> to store read bytes

    // Assure the telemetry line received is the correct length
    let mut message_len = 0;
    while message_len != 22 {
        let _ = reader.read_line(&mut telemetry).expect("Read failed !");
        message_len = telemetry.len();
    }

    let row = process_string(&telemetry);
    telemetry.clear();

    row
}

fn calibrate_zero(load_cell_devices: &mut Vec<load_cell::Hx711>) -> Vec<f64> {
    let mut statistics: Vec<(f64, f64)> = vec![(0.0, 0.0); load_cell_devices.len()];
    let average_time = 5;
    let mut readings_count = 0;

    let start_time = std::time::Instant::now();

    while start_time.elapsed().as_secs() < average_time {
        let load_cell_read = load_cell::Hx711::read_multiple(load_cell_devices);

        for i in 0..load_cell_read.len() {
            if let Ok(read_result) = load_cell_read
                .get(i)
                .expect("Tried to read from non-existent load cell")
            {
                if i >= statistics.len() {
                    statistics.push((0.0, 0.0));
                }

                let (sum, sum_of_squares) = &mut statistics[i];
                *sum += *read_result;
                *sum_of_squares += *read_result * *read_result;
            }
        }
        readings_count += 1;
    }

    for (i, (sum, sum_of_squares)) in statistics.iter_mut().enumerate() {
        let mut mean = *sum / (readings_count as f64);
        let mut variance = (*sum_of_squares) / (readings_count as f64) - (mean * mean);
        let coeff = -1.0 / CALIBRATION_GRADIENTS[i];

        mean = coeff * mean;
        variance = coeff * coeff * variance;

        *sum = mean;
        *sum_of_squares = variance;

        println!(
            "Load Cell {}: offset {:.4} stdev {:.4} g",
            i,
            mean,
            variance.sqrt()
        )
    }
    println!("");

    // Return the first element i.e. the mean
    statistics.iter().map(|&(first, _)| first).collect()
}
