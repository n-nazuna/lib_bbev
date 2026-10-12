use crate::blk_cmd::XferParameter;

#[derive(Debug)]
pub enum Cdb {
    Cdb32([u8; 32]),
    Cdb16([u8; 16]),
    Cdb12([u8; 12]),
    Cdb10([u8; 10]),
    Cdb6([u8; 6]),
}
pub struct Scsi {
    pub cdb: Cdb,
    pub xfer: XferParameter,
}
impl Cdb {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Cdb::Cdb32(cdb) => cdb,
            Cdb::Cdb16(cdb) => cdb,
            Cdb::Cdb12(cdb) => cdb,
            Cdb::Cdb10(cdb) => cdb,
            Cdb::Cdb6(cdb) => cdb,
        }
    }
}