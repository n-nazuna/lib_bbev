use crate::blk_cmd::{XferDirection, XferLength, XferParameter};

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

pub struct Read16 {
    fua: bool,
    lba: u64,
    transfer_length: u32,
    control: u8,
}
impl Read16 {
    pub fn new() -> Self {
        Read16 {
            fua: false,
            lba: 0,
            transfer_length: 0,
            control: 0,
        }
    }
    pub fn fua(mut self, fua: bool) -> Self {
        self.fua = fua;
        self
    }
    pub fn lba(mut self, lba: u64) -> Self {
        self.lba = lba;
        self
    }
    pub fn transfer_length(mut self, transfer_length: u32) -> Self {
        self.transfer_length = transfer_length;
        self
    }
    pub fn control(mut self, control: u8) -> Self {
        self.control = control;
        self
    }
    pub fn build(&self) -> Scsi {
        let mut cdb = [0u8; 16];
        cdb[0] = 0x88; // READ (16)
        cdb[1] = if self.fua { 0b100 } else { 0 };
        cdb[2] = (self.lba >> 56) as u8;
        cdb[3] = (self.lba >> 48) as u8;
        cdb[4] = (self.lba >> 40) as u8;
        cdb[5] = (self.lba >> 32) as u8;
        cdb[6] = (self.lba >> 24) as u8;
        cdb[7] = (self.lba >> 16) as u8;
        cdb[8] = (self.lba >> 8) as u8;
        cdb[9] = self.lba as u8;
        cdb[10] = (self.transfer_length >> 24) as u8;
        cdb[11] = (self.transfer_length >> 16) as u8;
        cdb[12] = (self.transfer_length >> 8) as u8;
        cdb[13] = self.transfer_length as u8;
        cdb[14] = 0;
        cdb[15] = self.control;
        Scsi {
            cdb: Cdb::Cdb16(cdb),
            xfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                XferLength::Sectors(self.transfer_length),
            )),
        }
    }
}
impl Default for Read16 {
    fn default() -> Self {
        Self::new()
    }
}
