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
pub struct Scsi {
    pub cdb: Cdb,
    pub xfer_param: XferParam,
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
        let t_dir = match self.ata_cmd.xfer_param.direction {
            XferDirection::TargetToInitiator => 1u8,
            XferDirection::InitiatorToTarget => 0u8,
            XferDirection::NoDataTransfer => 0u8,
        };
        let (byte_block, t_type) = match self.ata_cmd.xfer_param.protocol.length() {
            XferLength::Pages(_) => (1u8, 0u8),
            XferLength::Sectors(_) => (1u8, 0u8),
            XferLength::Bytes(_) => (0u8, 0u8),
            XferLength::None => (0u8, 0u8),
        };
        let t_length = match (
            self.ata_cmd.xfer_param.protocol.length(),
            self.ata_cmd.command as u8,
        ) {
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
impl AtaPt16 {
    fn cdb(&self) -> Cdb {
        let mut cdb = [0u8; 16];
        cdb[0] = 0x85; // ATA PASS-THROUGH (16)
        cdb[1] = self
            .ata_cmd
            .xfer_param
            .protocol
            .code(self.ata_cmd.xfer_param.direction)
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
        Cdb::cdb16(cdb)
    }
    fn xfer_length(&self) -> XferParam {
        XferParam {
            direction: self.ata_cmd.xfer_param.direction,
            length: self.ata_cmd.xfer_param.protocol.length(),
        }
    }
    pub fn build(&self) -> Scsi {
        Scsi {
            cdb: self.cdb(),
            xfer_param: self.xfer_length(),
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
            cdb: Cdb::cdb16(cdb),
            xfer_param: XferParam {
                direction: XferDirection::TargetToInitiator,
                length: XferLength::Sectors(self.transfer_length),
            },
        }
    }
}