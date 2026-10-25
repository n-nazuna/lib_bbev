//! READ DMA EXT の正常系サンプル。
//!
//! 使用例: cargo run --example ata -- /dev/sda

use lib_bbev::ata::{Command, CommandCode};
use lib_bbev::blk_cmd::XferLength;
use lib_bbev::scsi::Status;
use lib_bbev::scsi::sat::AtaPt;
use lib_bbev::sg_io::Device;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let device_path = env::args()
        .nth(1)
        .ok_or("usage: cargo run --example ata -- <device_path>")?;
    let device = Device::open(device_path)?;

    let ata = Command::builder()
        .lba(0)
        .count(1)
        .command(CommandCode::ReadDmaExt)
        .dma_data_in(XferLength::Sectors(1));
    let scsi = AtaPt::new(ata).cdb16();
    let mut buffer = device.allocate(&scsi)?;
    let response = device.execute(&scsi, &mut buffer)?;

    if response.status != Status::Good {
        return Err(format!("READ DMA EXT failed: {response:?}").into());
    }

    print!("First 16 bytes:");
    for byte in buffer.iter().take(16) {
        print!(" {byte:02X}");
    }
    println!();
    Ok(())
}
