mod ata;
mod scsi;
mod sg_io;
use ata::Ata;
fn main() {
    let cmd = ata::ReadDmaExt::new(.. count: 1, lba: 0x1000 ..);
    println!(
        "Hello, world! command: {:#X}, count: {:#X}, fis: {:02X?}",
        cmd.command(),
        cmd.count(),
        cmd.fis()
    );
}
