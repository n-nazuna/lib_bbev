//! 意図的に失敗する READ DMA EXT のサンプル。
//!
//! 使用例: cargo run --example err -- /dev/sda

use lib_bbev::ata::{Command, CommandCode};
use lib_bbev::scsi::{DescriptorSense, FixedSense, SenseData, sat::AtaPt};
use lib_bbev::sg_io::Device;
use std::env;
use std::error::Error;


fn print_fixed_sense(sense: &FixedSense) {
    println!("  Sense key: 0x{:02X}", sense.sense_key);
    println!(
        "  ASC: 0x{:02X}, ASCQ: 0x{:02X}",
        sense.additional_sense_code, sense.additional_sense_qualifier
    );
    println!(
        "  Flags: valid={}, filemark={}, eom={}, incorrect_length={}",
        sense.valid, sense.filemark, sense.end_of_medium, sense.incorrect_length
    );
    println!("  Information: {:02X?}", sense.information);
    println!(
        "  Command-specific information: {:02X?}",
        sense.command_specific_information
    );
    println!(
        "  Additional sense length: {}",
        sense.additional_sense_length
    );
    println!(
        "  Field replaceable unit code: 0x{:02X}",
        sense.field_replaceable_unit_code
    );
    println!("  Sense-key-specific: {:02X?}", sense.sense_key_specific);
    println!("  Additional sense: {:02X?}", sense.additional_sense);
}

fn print_descriptor_sense(sense: &DescriptorSense) {
    println!("  Sense key: 0x{:02X}", sense.sense_key);
    println!(
        "  ASC: 0x{:02X}, ASCQ: 0x{:02X}",
        sense.additional_sense_code, sense.additional_sense_qualifier
    );
    println!(
        "  Additional sense length: {}",
        sense.additional_sense_length
    );
    println!("  Descriptor bytes: {:02X?}", sense.sense_data_descriptor);
}

fn print_sense(sense: &SenseData) {
    match sense {
        SenseData::CurrentFixed(sense) => {
            println!("Sense format: current fixed (0x70)");
            print_fixed_sense(sense);
        }
        SenseData::DeferredFixed(sense) => {
            println!("Sense format: deferred fixed (0x71)");
            print_fixed_sense(sense);
        }
        SenseData::CurrentDescriptor(sense) => {
            println!("Sense format: current descriptor (0x72)");
            print_descriptor_sense(sense);
        }
        SenseData::DeferredDescriptor(sense) => {
            println!("Sense format: deferred descriptor (0x73)");
            print_descriptor_sense(sense);
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let device_path = env::args()
        .nth(1)
        .ok_or("usage: cargo run --example err -- <device_path>")?;
    let device = Device::open(device_path, 512, 128 * 1024)?;

    let ata = Command::builder()
        .command(CommandCode::Nop)
        .nondata();
    let sat = AtaPt::new(ata);
    let scsi = sat.cdb16();
    let mut buffer = device.allocate(&scsi)?;
    let response = device.execute(&scsi, &mut buffer)?;

    println!("SCSI status: {:?}", response.status);
    if let Some(sense) = &response.sense_data {
        print_sense(sense);
    } else {
        println!("Sense data: unavailable");
    }
    if let Some(ata) = sat.response(&response) {
        println!("ATA status: {:02X}, error: {:02X}", ata.status, ata.error);
    }

    Ok(())
}
