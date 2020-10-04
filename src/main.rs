extern crate sysfs_gpio;
extern crate json;

use std::fs::File;
use std::io::prelude::*;
use std::time::Duration;
use std::thread;
use std::env;

use image;

mod driver;

fn check_config_port(value: &json::JsonValue) -> u64 {
    match value {
        json::JsonValue::Number(num) => num.as_fixed_point_u64(0).unwrap(),
        _ => panic!("Value is not a number")
    }
}
fn check_config_string(value: &json::JsonValue) -> String {
    match value {
        json::JsonValue::String(s) => s.to_string(),
        json::JsonValue::Short(s) => s.to_string(),
        _ => panic!("Value is not a string (but what is it??)")
    }
}

fn main() -> std::io::Result<()> {
    println!("POP (Pop team epic On E-paper)");

    let args: Vec<String> = env::args().collect();
    let image =
        if args.len() < 2 {
            None
        } else {
            Some(image::open(&args[1]).unwrap().to_luma())
        };
    let red_image =
        if args.len() < 3 {
            None
        } else {
            let args: Vec<String> = env::args().collect();
            Some(image::open(&args[2]).unwrap().to_luma())
        };

    let mut file = File::open("config.json")?;
    let mut config_contents = String::new();
    file.read_to_string(&mut config_contents)?;

    let config = json::parse(&config_contents).unwrap();

    let rst_pin = check_config_port(&config["pins"]["RST"]);
    let dc_pin = check_config_port(&config["pins"]["DC"]);
    let busy_pin = check_config_port(&config["pins"]["BUSY"]);
    let spi_devname = check_config_string(&config["spi"]["dev"]);

    println!("Pin configuration: RST = {}, DC = {}, BUSY = {}", rst_pin, dc_pin, busy_pin);
    println!("SPI configuration: {}", spi_devname);

    let mut paper = driver::EPaper42Driver::new(busy_pin, rst_pin, dc_pin, &spi_devname);

    paper.init().unwrap();

    println!("1. Reset");
    paper.reset().unwrap();

    println!("2. First Sequence");
    paper.first_sequence().unwrap();

    match image {
        Some(image) => {
            println!("3. Print Image");
            if let Some(red_image) = red_image {
                paper.print_image(&image, Some(&red_image)).unwrap();
            } else {
                paper.print_image(&image, None).unwrap();
            }
        },
        None => {
            println!("3. Clear Display");
            paper.clear_display().unwrap();

            thread::sleep(Duration::from_millis(500));

            println!("4. Show Tricolor Stripe");
            paper.print_tricolor().unwrap();
        }
    }
    println!("Finished. Closing...");

    paper.close().unwrap();

    Ok(())
}
