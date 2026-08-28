use crate::sg_io::XferParam;
use crate::ata::Ata;
pub trait Scsi {
    fn cdb(&self) -> &[u8];
    fn xfer_length(&self) -> crate::sg_io::XferParam;
}
pub struct AtaPt16 {
    ata_cmd: Box<dyn Ata>,
}
impl Scsi for AtaPt16 {
    fn cdb(&self) -> &[u8] {
        &[0x85; 16]
    }
    fn xfer_length(&self) -> XferParam {
        XferParam {
            direction: crate::sg_io::XferDirection::TargetToInitiator,
            length: crate::sg_io::XferLength::Sectors(self.ata_cmd.count() as u32),
        }
    }
}