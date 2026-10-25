//! READ LOG DMA EXT の正常系サンプル。
//!
//! 使用例: cargo run --example gpl -- /dev/sda

use lib_bbev::ata::gpl::Gpl;
use lib_bbev::scsi::Status;
use lib_bbev::scsi::sat::AtaPt;
use lib_bbev::sg_io::Device;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let device_path = env::args()
        .nth(1)
        .ok_or("usage: cargo run --example gpl -- <device_path>")?;
    let device = Device::open(device_path)?;

    let ata = Gpl::builder()
        .address(0)
        .page_number(0)
        .page_count(1)
        .read_log_dma_ext();
    let sat = AtaPt::new(ata);
    let scsi = sat.cdb16();
    let mut buffer = device.allocate(&scsi)?;
    let response = device.execute(&scsi, &mut buffer)?;

    if response.status != Status::Good {
        return Err(format!("READ LOG DMA EXT failed: {response:?}").into());
    }

    print!("First 16 bytes:");
    for byte in buffer.iter().take(16) {
        print!(" {byte:02X}");
    }
    println!();

    Ok(())
}
