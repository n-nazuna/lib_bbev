use crate::sg_io::XferParam;
pub trait Scsi {
    fn cdb(&self) -> &[u8];
    fn xfer_length(&self) -> crate::sg_io::XferParam;
}
