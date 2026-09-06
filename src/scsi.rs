use crate::ata::Ata;
use crate::sg_io::{XferDirection, XferLength};
#[derive(Debug)]
pub enum Cdb {
    Cdb16([u8; 16]),
    Cdb12([u8; 12]),
    Cdb10([u8; 10]),
    Cdb6([u8; 6]),
}
pub struct Scsi {
    pub cdb: Cdb,
    pub xfer: XferDirection,
}
pub struct AtaPt16 {
    ata_cmd: Ata,
}
impl AtaPt16 {
    pub fn new(ata_cmd: Ata) -> Self {
        AtaPt16 { ata_cmd }
    }
    fn byte2(&self) -> u8 {
        let offline = 0u8;
        let ck_cond = 0u8;
        let xfer = self.ata_cmd.protocol.xfer();
        let t_dir = match xfer {
            XferDirection::TargetToInitiator(_) => 1u8,
            XferDirection::InitiatorToTarget(_) => 0u8,
            XferDirection::NoDataTransfer => 0u8,
        };
        let (byte_block, t_type) = match xfer {
            XferDirection::TargetToInitiator(length)
            | XferDirection::InitiatorToTarget(length) => match length {
            XferLength::Pages(_) => (1u8, 0u8),
            XferLength::Sectors(_) => (1u8, 0u8),
            XferLength::Bytes(_) => (0u8, 0u8),
            },
            XferDirection::NoDataTransfer => (0u8, 0u8),
        };
        let t_length = match (xfer, self.ata_cmd.command as u8) {
            (XferDirection::NoDataTransfer, _) => 0b00,
            (XferDirection::TargetToInitiator(XferLength::Pages(_)), 0x66)
            | (XferDirection::InitiatorToTarget(XferLength::Pages(_)), 0x66) => 0b11,
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
impl AtaPt16 {
    fn cdb(&self) -> Cdb {
        let mut cdb = [0u8; 16];
        cdb[0] = 0x85; // ATA PASS-THROUGH (16)
        cdb[1] = self
            .ata_cmd
            .protocol
            .code()
            << 1
            | 1;
        cdb[2] = self.byte2();
        cdb[3] = (self.ata_cmd.feature >> 8) as u8;
        cdb[4] = self.ata_cmd.feature as u8;
        cdb[5] = (self.ata_cmd.count >> 8) as u8;
        cdb[6] = self.ata_cmd.count as u8;
        cdb[7] = (self.ata_cmd.lba >> 24) as u8;
        cdb[8] = self.ata_cmd.lba as u8;
        cdb[9] = (self.ata_cmd.lba >> 32) as u8;
        cdb[10] = (self.ata_cmd.lba >> 8) as u8;
        cdb[11] = (self.ata_cmd.lba >> 40) as u8;
        cdb[12] = (self.ata_cmd.lba >> 16) as u8;
        cdb[13] = self.ata_cmd.device;
        cdb[14] = self.ata_cmd.command as u8;
        cdb[15] = self.ata_cmd.control;
        Cdb::Cdb16(cdb)
    }
    fn xfer(&self) -> XferDirection {
        self.ata_cmd.protocol.xfer()
    }
    pub fn build(&self) -> Scsi {
        Scsi {
            cdb: self.cdb(),
            xfer: self.xfer(),
        }
    }
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
            xfer: XferDirection::TargetToInitiator(XferLength::Sectors(
                self.transfer_length,
            )),
        }
    }
}
impl Default for Read16 {
    fn default() -> Self {
        Self::new()
    }
}