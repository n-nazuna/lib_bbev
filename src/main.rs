mod ata;
mod scsi;
mod sg_io;
use ata::Ata;
fn main() {
    let cmd = ata::ReadLogDmaExt::new(0x01, 0x00, 0x0001);
    println!(
        "Hello, world! command: {:#X}, count: {:#X}, fis: {:02X?}",
        cmd.command(),
        cmd.count(),
        ata::fis(&cmd)
    );
}
