mod ata;
mod scsi;
mod sg_io;
use ata::{Ata, ReadLogExt};
fn main() {
    let cmd = ReadLogExt {
        log_page_count: 0x10,
        log_address: 0x01,
        page_number: 0x0001,
    };
    println!(
        "Hello, world! command: {:#X}, count: {:#X}",
        cmd.command(),
        cmd.count()
    );
}
