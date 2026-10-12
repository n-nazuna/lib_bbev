use crate::ata::{Ata, AtaProtocol};
use crate::blk_cmd::{XferDirection, XferLength, XferParameter};
use crate::scsi::{Cdb, Scsi};
pub struct AtaPt {
    ata_cmd: Ata,
}
impl AtaPt {
    pub fn new(ata_cmd: Ata) -> Self {
        AtaPt { ata_cmd }
    }
    pub fn code(&self) -> u8 {
        match &self.ata_cmd.protocol {
            AtaProtocol::NonData => 0x3,
            AtaProtocol::Pio { direction, .. } => match direction {
                XferDirection::TargetToInitiator(_) => 0x4,
                XferDirection::InitiatorToTarget(_) => 0x5,
            },
            AtaProtocol::Dma { .. } => 0x6,
            AtaProtocol::NcqNonData | AtaProtocol::Ncq { .. } => 0xC,
        }
    }
    fn byte2(&self) -> u8 {
        let offline = 0u8;
        let ck_cond = 0u8;
        let xfer = self.xfer();
        let t_dir = match xfer {
            XferParameter::XferDirection(XferDirection::TargetToInitiator(_)) => 1u8,
            XferParameter::XferDirection(XferDirection::InitiatorToTarget(_)) => 0u8,
            XferParameter::NoDataTransfer => 0u8,
        };
        let (byte_block, t_type) = match xfer {
            XferParameter::XferDirection(XferDirection::TargetToInitiator(length))
            | XferParameter::XferDirection(XferDirection::InitiatorToTarget(length)) => {
                match length {
                    XferLength::Pages(_) => (1u8, 0u8),
                    XferLength::Sectors(_) => (1u8, 1u8),
                    XferLength::Bytes(_) => (0u8, 0u8),
                }
            }
            XferParameter::NoDataTransfer => (1u8, 0u8),
        };
        let t_length = match (xfer, self.ata_cmd.command as u8) {
            (XferParameter::NoDataTransfer, _) => 0b00,
            (
                XferParameter::XferDirection(XferDirection::TargetToInitiator(XferLength::Pages(
                    _,
                ))),
                0x66,
            )
            | (
                XferParameter::XferDirection(XferDirection::InitiatorToTarget(XferLength::Pages(
                    _,
                ))),
                0x66,
            ) => 0b11,
            (_, 0x60 | 0x61 | 0x63 | 0x64 | 0x65) => 0b01, // FPDMA
            (_, _) => 0b10,
        };
        (offline << 6)
            | (ck_cond << 5)
            | (t_type << 4)
            | (t_dir << 3)
            | (byte_block << 2)
            | t_length
    }
}
impl AtaPt {
    fn build_cdb16(&self) -> Cdb {
        let mut cdb = [0u8; 16];
        cdb[0] = 0x85; // ATA PASS-THROUGH (16)
        cdb[1] = self.code() << 1 | 1;
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
    fn build_cdb32(&self) -> Cdb {
        let mut cdb = [0u8; 32];
        cdb[0] = 0x7f; // ATA PASS-THROUGH (32)
        cdb[1] = self.ata_cmd.control;
        cdb[2..6].copy_from_slice(&[0u8; 4]);
        cdb[7] = 0x18; // ADDITIONAL CDB LENGTH
        cdb[8..10].copy_from_slice(&0x1FF0u16.to_be_bytes()); // SERVICE ACTION
        cdb[10] = self.code() << 1 | 1;
        cdb[11] = self.byte2();
        cdb[12..14].copy_from_slice(&[0u8; 2]);
        cdb[14..20].copy_from_slice(&self.ata_cmd.lba.to_be_bytes()[2..8]);
        cdb[20..22].copy_from_slice(&self.ata_cmd.feature.to_be_bytes());
        cdb[22..24].copy_from_slice(&self.ata_cmd.count.to_be_bytes());
        cdb[24] = self.ata_cmd.device;
        cdb[25] = self.ata_cmd.command as u8;
        cdb[26] = 0;
        cdb[27] = self.ata_cmd.icc;
        cdb[28..32].copy_from_slice(&self.ata_cmd.aux.to_be_bytes());

        Cdb::Cdb32(cdb)
    }
    fn xfer(&self) -> XferParameter {
        match &self.ata_cmd.protocol {
            AtaProtocol::NonData | AtaProtocol::NcqNonData => XferParameter::NoDataTransfer,
            AtaProtocol::Pio { direction }
            | AtaProtocol::Dma { direction }
            | AtaProtocol::Ncq { direction } => XferParameter::XferDirection(*direction),
        }
    }
    pub fn cdb16(&self) -> Scsi {
        Scsi {
            cdb: self.build_cdb16(),
            xfer: self.xfer(),
        }
    }
    pub fn cdb32(&self) -> Scsi {
        Scsi {
            cdb: self.build_cdb32(),
            xfer: self.xfer(),
        }
    }
}
