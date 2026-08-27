use crate::sg_io::{XferParam, XferDirection, XferLength};
pub trait Ata {
    fn command(&self) -> u8;
    fn feature(&self) -> u16;
    fn count(&self) -> u16;
    fn lba(&self) -> u64;
    fn icc(&self) -> u8;
    fn aux(&self) -> u16;
    fn device(&self) -> u8;
    fn xfer_length(&self) -> XferParam;
}
pub struct ReadLogExt {
    pub log_page_count: u16,
    pub log_address: u8,
    pub page_number: u16,
}
impl Ata for ReadLogExt {
    fn command(&self) -> u8 {
        0x2F
    }
    fn feature(&self) -> u16 {
        0
    }
    fn count(&self) -> u16 {
        self.log_page_count
    }
    fn lba(&self) -> u64 {
        ((self.page_number as u64 & 0xFF00) << 24)
            | ((self.page_number as u64 & 0xFF) << 8)
            | (self.log_address as u64)
    }
    fn icc(&self) -> u8 {
        0
    }
    fn aux(&self) -> u16 {
        0
    }
    fn device(&self) -> u8 {
        0
    }
    fn xfer_length(&self) -> XferParam {
        XferParam {
            direction: crate::sg_io::XferDirection::TargetToInitiator,
            length: crate::sg_io::XferLength::Pages(self.log_page_count as u32),
        }
    }
}

pub struct WriteLogExt {
    pub log_page_count: u16,
    pub log_address: u8,
    pub page_number: u16,
}
impl Ata for WriteLogExt {
    fn command(&self) -> u8 {
        0x3F
    }
    fn feature(&self) -> u16 {
        0
    }
    fn count(&self) -> u16 {
        self.log_page_count
    }
    fn lba(&self) -> u64 {
        ((self.page_number as u64 & 0xFF00) << 24)
            | ((self.page_number as u64 & 0xFF) << 8)
            | (self.log_address as u64)
    }
    fn icc(&self) -> u8 {
        0
    }
    fn aux(&self) -> u16 {
        0
    }
    fn device(&self) -> u8 {
        0
    }
    fn xfer_length(&self) -> XferParam {
        XferParam {
            direction: crate::sg_io::XferDirection::InitiatorToTarget,
            length: crate::sg_io::XferLength::Pages(self.log_page_count as u32),
        }
    }
}
