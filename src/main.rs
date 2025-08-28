use orbita2d_controller::Orbita2dController;

use serde::Deserialize;
use serde::Serialize;

use chrono::prelude::*;
use clap::Parser;
use std::time::SystemTime;
use std::{error::Error, thread, time::Duration};

use poulpe_ethercat_grpc::server::launch_server;

/// Orbita2d testbench program
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Orbita2d configuration file
    #[arg(short, long, default_value = "config/fake.yaml")]
    configfile: String,

    /// Should we start the grpc_server to run the program in standalone
    #[arg(short, long)]
    start_server: bool,

    /// Input csv with motion to follow
    #[arg(short, long)]
    input_csv: Option<String>,

    /// Result output csv
    #[arg(short, long)]
    output_csv: Option<String>,

    /// Should we start the live viewer
    #[arg(short, long, default_value = "false")]
    viewer: bool,

    /// Should we start/end at the zero position
    #[arg(short, long, default_value = "true")]
    zero: bool,

    /// How many loop should we perform
    #[arg(short, long, default_value = "1")]
    nb_loop: u16,
}

#[derive(Debug, Deserialize)]
// #[serde(rename_all = "PascalCase")]
struct Input {
    timestamp: f64,
    torque_on: bool,
    target_ring: f64,
    target_center: f64,
    velocity_limit_a: f64,
    velocity_limit_b: f64,
    torque_limit_a: f64,
    torque_limit_b: f64,
}

#[derive(Debug, Serialize)]
struct Output {
    timestamp: f64,
    torque_on: bool,
    present_ring: f64,
    present_center: f64,
    target_ring: f64,
    target_center: f64,
    present_velocity_ring: f64,
    present_velocity_center: f64,
    present_torque_ring: f64,
    present_torque_center: f64,
    present_pos_a: f64,
    present_pos_b: f64,
    present_velocity_a: f64,
    present_velocity_b: f64,
    present_current_a: f64,
    present_current_b: f64,
    present_temperature_a: f64,
    present_temperature_b: f64,
    axis_sensor_ring: f64,
    axis_sensor_center: f64,
    axis_zeros_ring: f64,
    axis_zeros_center: f64,
    board_temperature_a: f64,
    board_temperature_b: f64,
    board_state: u8,
    control_mode: u8,
}

use rprompt::prompt_reply;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let args = Args::parse();

    let rec = if args.viewer {
        let _rec = rerun::RecordingStreamBuilder::new("Test Orbita2d").spawn()?;
        Some(_rec)
    } else {
        None
    };

    if args.start_server {
        log::info!("Starting the server");
        // run in a thread, do not block main thread
        thread::spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(launch_server("config/ethercat.yaml"))
                .unwrap();
        });
        thread::sleep(Duration::from_secs(2));
    }

    log::info!("Config file: {}", args.configfile);

    let infile_path = match args.input_csv {
        Some(s) => {
            log::info!("Input csv file: {:?}", s);
            s
        }
        None => {
            log::warn!("No input csv file provided");
            let buffer =
                prompt_reply("Please enter the input csv file path [input.csv]: ").unwrap();
            if buffer.trim().is_empty() {
                "input.csv".to_string()
            } else {
                buffer.trim().to_string()
            }
        }
    };

    let outfile_path = match args.output_csv {
        Some(s) => s,
        None => {
            log::warn!("No output csv file provided");
            let buffer =
                prompt_reply("Please enter the output csv file path [output.csv]: ").unwrap();
            if buffer.trim().is_empty() {
                "output.csv".to_string()
            } else {
                buffer.trim().to_string()
            }
        }
    };

    let mut controller = Orbita2dController::with_config(&args.configfile)?;
    let t = controller.is_torque_on();
    match t {
        Ok(t) => log::info!("Torque is {}", t),
        Err(e) => log::error!("Error: {}", e),
    }
    let t = controller.disable_torque(); //Start with torque_off
    match t {
        Ok(_) => {}
        Err(e) => log::error!("Error: {}", e),
    }
    // let date_as_string = Utc::now().to_string();
    let current_localtime = Local::now();
    let date_as_string = current_localtime.format("%Y-%m-%d_%Hh%Mm%Ss");
    thread::sleep(Duration::from_millis(1000));

    let mut iteration: u16 = 1;
    // let mut input_csv = csv::Reader::from_reader(infile);
    // let startpos = input_csv.position().clone();
    let mut pos = csv::Position::new();

    {
        let infile = match std::fs::File::open(&infile_path) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Error opening input csv file: {}", e);
                return Err(e.into());
            }
        };
        let input_csv = csv::Reader::from_reader(infile);
        // let mut startpos = pos.set_line(2_u64);
        // let mut startpos = pos.set_record(2_u64);
        // let mut startpos = pos.set_byte(218);
        let mut iter = input_csv.into_records();

        for _ in 0..2 {
            // horrible trick to get the position of the first data for later rewind
            pos = iter.reader().position().clone();
            iter.next();
        }
    }
    let startpos = pos;
    let infile = match std::fs::File::open(&infile_path) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Error opening input csv file: {}", e);
            return Err(e.into());
        }
    };
    let mut input_csv = csv::Reader::from_reader(infile);

    if args.zero {
        controller.enable_torque(true)?;
        thread::sleep(Duration::from_millis(1000));
        controller.set_target_orientation([0.0, 0.0])?;
        thread::sleep(Duration::from_millis(1000));
        controller.disable_torque()?;
        thread::sleep(Duration::from_millis(10));
    }

    while iteration < args.nb_loop + 1 {
        let now = SystemTime::now();
        log::info!("Iteration: {iteration}/{:?}", args.nb_loop);
        let mut all_data: Vec<Output> = Vec::new();

        for in_csv in input_csv.deserialize() {
            let t = now.elapsed().unwrap().as_secs_f64();
            let input_csv_data: Input = in_csv?;
            log::debug!("INPUT: {:?}", input_csv_data);

            //Read feedback from Orbita
            let curr_ori = controller.get_current_orientation()?;
            let torque = controller.is_torque_on()?;
            let curr_vel = controller.get_current_velocity()?;
            let curr_torque = controller.get_current_torque()?;
            let curr_raw_vel = controller.get_raw_motors_velocity()?;
            let curr_raw_torque = controller.get_raw_motors_current()?;
            let curr_raw_pos = controller.get_raw_motors_position()?;
            let curr_temp = controller.get_raw_motors_temperature()?;
            let curr_axis = controller.get_axis_sensors()?;
            let curr_state = controller.get_board_state()?;
            let axis_zeros = controller.get_axis_sensor_zeros()?;
            let board_temp = controller.get_raw_boards_temperature()?;
            let control_mode = controller.get_control_mode()?;
            all_data.push(Output {
                timestamp: t,
                torque_on: torque,
                present_ring: curr_ori[0],
                present_center: curr_ori[1],
                target_ring: input_csv_data.target_ring,
                target_center: input_csv_data.target_center,
                present_velocity_ring: curr_vel[0],
                present_velocity_center: curr_vel[1],
                present_torque_ring: curr_torque[0],
                present_torque_center: curr_torque[1],
                present_pos_a: curr_raw_pos[0],
                present_pos_b: curr_raw_pos[1],
                present_velocity_a: curr_raw_vel[0],
                present_velocity_b: curr_raw_vel[1],
                present_current_a: curr_raw_torque[0],
                present_current_b: curr_raw_torque[1],
                present_temperature_a: curr_temp[0],
                present_temperature_b: curr_temp[1],
                axis_sensor_ring: curr_axis[0],
                axis_sensor_center: curr_axis[1],
                axis_zeros_ring: axis_zeros[0],
                axis_zeros_center: axis_zeros[1],
                board_temperature_a: board_temp[0],
                board_temperature_b: board_temp[1],
                board_state: curr_state,
                control_mode: control_mode[0],
            });

            let tosleep = (input_csv_data.timestamp - t) * 1000.0;
            thread::sleep(Duration::from_millis(tosleep as u64));

            //Write commands to Orbita
            if input_csv_data.torque_on {
                controller.enable_torque(true)?;
            } else {
                controller.disable_torque()?;
            }
            controller.set_target_orientation([
                input_csv_data.target_ring,
                input_csv_data.target_center,
            ])?;

            controller.set_raw_motors_velocity_limit([
                input_csv_data.velocity_limit_a,
                input_csv_data.velocity_limit_b,
            ])?;

            controller.set_raw_motors_torque_limit([
                input_csv_data.torque_limit_a,
                input_csv_data.torque_limit_b,
            ])?;

            // Rerun
            if let Some(rec) = &rec {
                rec.set_time_seconds("timestamp", t);
                rec.log(
                    "target/torque_on",
                    &rerun::Scalar::new(if input_csv_data.torque_on { 1.0 } else { 0.0 }),
                )?;
                rec.log("target/board_state", &rerun::Scalar::new(curr_state as f64))?;
                rec.log(
                    "target/control_mode",
                    &rerun::Scalar::new(control_mode[0] as f64),
                )?;

                rec.log(
                    "position/target/ring",
                    &rerun::Scalar::new(input_csv_data.target_ring),
                )?;
                rec.log(
                    "position/target/center",
                    &rerun::Scalar::new(input_csv_data.target_center),
                )?;

                rec.log("position/present/ring", &rerun::Scalar::new(curr_ori[0]))?;
                rec.log("position/present/center", &rerun::Scalar::new(curr_ori[1]))?;

                rec.log("position/raw/A", &rerun::Scalar::new(curr_raw_pos[0]))?;
                rec.log("position/raw/B", &rerun::Scalar::new(curr_raw_pos[1]))?;

                rec.log("velocity/present/ring", &rerun::Scalar::new(curr_vel[0]))?;
                rec.log("velocity/present/center", &rerun::Scalar::new(curr_vel[1]))?;

                rec.log("velocity/raw/A", &rerun::Scalar::new(curr_raw_vel[0]))?;
                rec.log("velocity/raw/B", &rerun::Scalar::new(curr_raw_vel[1]))?;

                rec.log("torque/present/ring", &rerun::Scalar::new(curr_torque[0]))?;
                rec.log("torque/present/center", &rerun::Scalar::new(curr_torque[1]))?;

                rec.log("torque/raw/A", &rerun::Scalar::new(curr_raw_torque[0]))?;
                rec.log("torque/raw/B", &rerun::Scalar::new(curr_raw_torque[1]))?;

                rec.log(
                    "position/axis_sensor/ring",
                    &rerun::Scalar::new(curr_axis[0]),
                )?;
                rec.log(
                    "position/axis_sensor/center",
                    &rerun::Scalar::new(curr_axis[1]),
                )?;

                rec.log(
                    "limits/velocity/A",
                    &rerun::Scalar::new(input_csv_data.velocity_limit_a),
                )?;
                rec.log(
                    "limits/velocity/B",
                    &rerun::Scalar::new(input_csv_data.velocity_limit_b),
                )?;

                rec.log(
                    "limits/torque/A",
                    &rerun::Scalar::new(input_csv_data.torque_limit_a),
                )?;
                rec.log(
                    "limits/torque/B",
                    &rerun::Scalar::new(input_csv_data.torque_limit_b),
                )?;

                rec.log("temperature/motor/A", &rerun::Scalar::new(curr_temp[0]))?;
                rec.log("temperature/motor/B", &rerun::Scalar::new(curr_temp[1]))?;

                rec.log("temperature/board/A", &rerun::Scalar::new(board_temp[0]))?;
                rec.log("temperature/board/B", &rerun::Scalar::new(board_temp[1]))?;
            }
        }

        if args.zero {
            controller.enable_torque(true)?;
            thread::sleep(Duration::from_millis(1000));
            controller.set_target_orientation([0.0, 0.0])?;
            thread::sleep(Duration::from_millis(1000));
            controller.disable_torque()?;
            thread::sleep(Duration::from_millis(10));
        }

        let torque = controller.disable_torque();
        match torque {
            Ok(_) => log::info!("Torque is off"),
            Err(e) => log::error!("Error: {}", e),
        }
        thread::sleep(Duration::from_millis(1000));

        if args.nb_loop > 1 {
            let outfile_it = format!("{outfile_path}_{date_as_string}_{iteration}");
            log::info!("Writing output csv file: {}", outfile_it);
            let outfile = match std::fs::File::create_new(&outfile_it) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Error opening output csv file: {}", e);
                    return Err(e.into());
                }
            };
            let mut output_csv = csv::Writer::from_writer(outfile);
            for data in all_data {
                output_csv.serialize(data)?;
            }
        } else {
            let outfile_it = format!("{outfile_path}_{date_as_string}");
            log::info!("Writing output csv file: {}", outfile_it);
            let outfile = match std::fs::File::create_new(&outfile_it) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Error opening output csv file: {}", e);
                    return Err(e.into());
                }
            };
            let mut output_csv = csv::Writer::from_writer(outfile);
            for data in all_data {
                output_csv.serialize(data)?;
            }
        }

        iteration += 1;
        input_csv.seek(startpos.clone())?;
    }

    Ok(())
}
