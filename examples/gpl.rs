//! GPLログをDMAで読み取り、xxd風に表示する。
//! 使用例: gpl /dev/sda -a 0x10 -n 0 -c 1

use clap::Parser;
use lib_bbev::ata::gpl::Gpl;
use lib_bbev::scsi::Status;
use lib_bbev::scsi::sat::AtaPt;
use lib_bbev::sg_io::Device;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::num::NonZeroU16;
use std::os::fd::AsRawFd;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gpl", about = "Read a GPL log and display a hex dump.")]
#[command(after_help = "Numbers accept decimal or hexadecimal with a 0x prefix.")]
struct Args {
    /// Device path
    device: PathBuf,

    /// Log address (0..=255)
    #[arg(short, long, default_value = "0", value_parser = parse_number::<u8>)]
    address: u8,

    /// Page number (0..=65535)
    #[arg(short, long, default_value = "0", value_parser = parse_number::<u16>)]
    number: u16,

    /// Page count (1..=65535)
    #[arg(short, long, default_value = "1", value_parser = parse_number::<NonZeroU16>)]
    count: NonZeroU16,
}

fn parse_number<T: TryFrom<u16>>(value: &str) -> Result<T, String> {
    let (digits, radix) = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => (hex, 16),
        None => (value, 10),
    };
    let number = u16::from_str_radix(digits, radix).map_err(|error| error.to_string())?;
    T::try_from(number).map_err(|_| "value is outside the allowed range".into())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&args.device)?;
    // GPLは固定512バイトのページ単位。転送上限はこの例では1024 KiB。
    let device = Device::new(file.as_raw_fd(), 512, 1024);

    let ata = Gpl::builder()
        .address(args.address)
        .page_number(args.number)
        .page_count(args.count.get())
        .feature(0)
        .read_log_dma_ext();
    let sat = AtaPt::new(ata);
    let scsi = sat.cdb16();
    let mut buffer = device.allocate(&scsi)?;

    let response = device.execute(&scsi, &mut buffer)?;
    if let Some(ata_response) = sat.response(&response) {
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
    if response.status != Status::Good {
        return Err(format!("SCSI command failed: {response:?}").into());
    }

    let mut output = io::BufWriter::new(io::stdout().lock());
    for (line, bytes) in buffer.chunks(16).enumerate() {
        write!(output, "{:08x}: ", line * 16)?;
        for index in 0..16 {
            if let Some(byte) = bytes.get(index) {
                write!(output, "{byte:02x}")?;
            } else {
                write!(output, "  ")?;
            }
            if index % 2 == 1 {
                write!(output, " ")?;
            }
        }
        write!(output, " ")?;
        for &byte in bytes {
            let character = if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '.'
            };
            write!(output, "{character}")?;
        }
        writeln!(output)?;
    }
    output.flush()?;

    Ok(())
}
