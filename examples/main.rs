use lib_bbev::sg_io::Device;
use lib_bbev::{ata, scsi};
use std::env;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <device_path>", args[0]);
        std::process::exit(1);
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&args[1])
        .expect("Failed to open device");

    let fd = file.as_raw_fd();
    let dev = Device::new(fd, 512, 1024);
    let cmd = ata::Gpl::new().page_count(1).address(0).read_log_dma_ext();
    let cdb = scsi::AtaPt16::new(cmd).build();

    let mut buf = dev.allocate(&cdb).expect("Failed to allocate buffer");

    match dev.execute(&cdb, &mut buf) {
        Ok(()) => {
            println!("Success! First 32 bytes:");
            for (i, b) in buf.iter().take(32).enumerate() {
                print!("{:02X} ", b);
                if (i + 1) % 16 == 0 {
                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
        }
    }
}
