mod ata;
mod scsi;
mod sg_io;
use scsi::Cdb;

use crate::scsi::Scsi;
fn main() {
    let cmd = ata::ReadLogDmaExt::new(0x01, 0x30, 0x0000);
    let cdb = scsi::AtaPt16::new(cmd);
    for i in 0..16 {
        let cdb_bytes = match cdb.cdb() {
            Cdb::cdb16(bytes) => bytes,
            _ => [0u8; 16],
        };
        print!("{:02X}", cdb_bytes[i]);
    }
}
