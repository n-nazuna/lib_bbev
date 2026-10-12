//! GPLログをDMAで読み書きする。設定と書き込みデータをmain内で変更する。

use lib_bbev::ata::gpl::Gpl;
use lib_bbev::scsi::sat::AtaPt;
use lib_bbev::scsi::{Response, SenseData, Status};
use lib_bbev::sg_io::{Device, SgIoErrorKind};
use std::error::Error;
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;

fn main() -> Result<(), Box<dyn Error>> {
    let device_path = "/dev/sg0";
    let log_address = 0xFF;
    let page_number = 0;
    let page_count = 1;
    let write = false; // false: READ LOG、true: WRITE LOG

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(device_path)?;
    // GPLは固定512バイトのページ単位。転送上限はこの例では1024 KiB。
    let device = Device::new(file.as_raw_fd(), 512, 1024);

    let ata = if write {
        // WRITE LOG DMA EXT
        Gpl::builder()
            .address(log_address)
            .page_number(page_number)
            .page_count(page_count)
            .write_log_dma_ext()
    } else {
        // READ LOG DMA EXT
        Gpl::builder()
            .address(log_address)
            .page_number(page_number)
            .page_count(page_count)
            .feature(0)
            .read_log_dma_ext()
    };
    let sat = AtaPt::new(ata);
    let command = sat.cdb16();
    let mut buffer = device.allocate(&command)?;
    if write {
        buffer.fill(0x00); // ここで書き込みデータを設定する。
    }

    if let Err(error) = device.execute(&command, &mut buffer) {
        // 現在のsg_io APIではCHECK CONDITIONのsenseはエラーに含まれる。
        if let SgIoErrorKind::ScsiError {
            status: 0x02,
            host_status: 0,
            sense,
            ..
        } = &error.kind
        {
            let scsi_response = Response {
                status: Status::CheckCondition,
                sense_data: SenseData::from_bytes(sense),
            };
            if let Some(ata_response) = sat.response(&scsi_response) {
                eprintln!(
                    "ATA STATUS={:02x}, ERROR={:02x}",
                    ata_response.status, ata_response.error
                );
                if let Some(register) = ata_response.register {
                    eprintln!(
                        "COUNT={:04x}, LBA={:012x}, DEVICE={:02x}",
                        register.count, register.lba, register.device
                    );
                }
            }
        }
        return Err(error.into());
    }

    // GOOD応答にはATAレジスタ情報が含まれない。
    if write {
        println!("Wrote {} bytes", buffer.len());
    } else {
        println!("{buffer:02x?}");
    }

    Ok(())
}
