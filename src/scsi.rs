use crate::ata::Ata;
use crate::sg_io::{XferDirection, XferLength};

#[derive(Copy, Clone)]
pub struct XferParam {
    pub direction: XferDirection,
    pub length: XferLength,
}
#[derive(Debug)]
pub enum Cdb {
    cdb16([u8; 16]),
    cdb12([u8; 12]),
    cdb10([u8; 10]),
    cdb6([u8; 6]),
}
pub trait Scsi {
    fn cdb(&self) -> Cdb;
    fn xfer_length(&self) -> XferParam;
}
pub struct AtaPt16<T: Ata> {
    ata_cmd: T,
}
impl<T: Ata> AtaPt16<T> {
    pub fn new(ata_cmd: T) -> Self {
        AtaPt16 { ata_cmd }
    }
    fn byte2(&self) -> u8 {
        let offline = 0u8;
        let ck_cond = 0u8;
        let t_dir = match self.ata_cmd.xfer_length().direction {
            XferDirection::TargetToInitiator => 1u8,
            XferDirection::InitiatorToTarget => 0u8,
            XferDirection::NoDataTransfer => 0u8,
        };
        let (byte_block, t_type) = match self.ata_cmd.xfer_length().length {
            XferLength::Pages(_) => (1u8, 0u8),
            XferLength::Sectors(_) => (1u8, 0u8),
            XferLength::Bytes(_) => (0u8, 0u8),
            XferLength::None => (0u8, 0u8),
        };
        let t_length = match (self.ata_cmd.xfer_length().length, self.ata_cmd.command()) {
            (XferLength::None, _) => 0b00,
            (XferLength::Pages(_), 0x66) => 0b11, // WRITE GATHERED EXT
            (_, 0x60 | 0x61 | 0x63 | 0x65) => 0b01, // FPDMA
            (_, _) => 0b10,
        };
        (offline << 7)
            | (ck_cond << 6)
            | (t_type << 5)
            | (t_dir << 4)
            | (byte_block << 2)
            | t_length
    }
}
impl<T: Ata> Scsi for AtaPt16<T> {
    fn cdb(&self) -> Cdb {
        let mut cdb = [0u8; 16];
        cdb[0] = 0x85; // ATA PASS-THROUGH (16)
        cdb[1] = (self.ata_cmd.protocol()) << 1 | 1;
        cdb[2] = self.byte2();
        cdb[3] = (self.ata_cmd.feature() >> 8) as u8;
        cdb[4] = self.ata_cmd.feature() as u8;
        cdb[5] = (self.ata_cmd.count() >> 8) as u8;
        cdb[6] = self.ata_cmd.count() as u8;
        cdb[7] = (self.ata_cmd.lba() >> 24) as u8;
        cdb[8] = self.ata_cmd.lba() as u8;
        cdb[9] = (self.ata_cmd.lba() >> 32) as u8;
        cdb[10] = (self.ata_cmd.lba() >> 8) as u8;
        cdb[11] = (self.ata_cmd.lba() >> 40) as u8;
        cdb[12] = (self.ata_cmd.lba() >> 16) as u8;
        cdb[13] = self.ata_cmd.device();
        cdb[14] = self.ata_cmd.command();
        cdb[15] = self.ata_cmd.control();
        Cdb::cdb16(cdb)
    }
    fn xfer_length(&self) -> XferParam {
        XferParam {
            direction: self.ata_cmd.xfer_length().direction,
            length: self.ata_cmd.xfer_length().length,
        }
    }
}
