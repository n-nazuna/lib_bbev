use crate::ata::Ata;
use crate::ata::AtaProtocol;
use crate::sg_io::{XferDirection, XferLength, XferParameter};
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
        let xfer = self.ata_cmd.protocol.xfer();
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
                    XferLength::Sectors(_) => (1u8, 0u8),
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
        cdb[0] = 0x7f; // ATA PASS-THROUGH (16)
        cdb[1] = self.ata_cmd.control as u8;
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
        cdb[27] = self.ata_cmd.icc as u8;
        cdb[28..32].copy_from_slice(&self.ata_cmd.aux.to_be_bytes());

        Cdb::Cdb32(cdb)
    }
    fn xfer(&self) -> XferParameter {
        self.ata_cmd.protocol.xfer()
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

#[cfg(test)]
mod tests {
    use super::{AtaPt, Cdb, Read16};
    use crate::ata::{Ata, AtaCmd, AtaProtocol};
    use crate::sg_io::{XferDirection, XferLength, XferParameter};

    fn ata_with_protocol(protocol: AtaProtocol) -> Ata {
        Ata::new().protocol(protocol)
    }

    #[test]
    fn code_maps_each_protocol_to_its_sat3_value() {
        assert_eq!(
            AtaPt::new(ata_with_protocol(AtaProtocol::NonData)).code(),
            0x3
        );
        assert_eq!(
            AtaPt::new(ata_with_protocol(AtaProtocol::Pio {
                direction: XferDirection::TargetToInitiator(XferLength::Sectors(1)),
            }))
            .code(),
            0x4
        );
        assert_eq!(
            AtaPt::new(ata_with_protocol(AtaProtocol::Pio {
                direction: XferDirection::InitiatorToTarget(XferLength::Sectors(1)),
            }))
            .code(),
            0x5
        );
        assert_eq!(
            AtaPt::new(ata_with_protocol(AtaProtocol::Dma {
                direction: XferDirection::TargetToInitiator(XferLength::Sectors(1)),
            }))
            .code(),
            0x6
        );
        assert_eq!(
            AtaPt::new(ata_with_protocol(AtaProtocol::NcqNonData)).code(),
            0xC
        );
        assert_eq!(
            AtaPt::new(ata_with_protocol(AtaProtocol::Ncq {
                direction: XferDirection::TargetToInitiator(XferLength::Sectors(1)),
            }))
            .code(),
            0xC
        );
    }

    #[test]
    fn build_encodes_ata_pass_through_16_cdb_and_xfer() {
        let ata_cmd = Ata::new()
            .feature(0x1234)
            .count(0x5678)
            .lba(0x0102_0304_0506)
            .control(0xBB)
            .device(0xAA)
            .command(AtaCmd::ReadLogDmaExt)
            .protocol(AtaProtocol::Dma {
                direction: XferDirection::TargetToInitiator(XferLength::Sectors(1)),
            });

        let scsi = AtaPt::new(ata_cmd).cdb16();

        let Cdb::Cdb16(cdb) = scsi.cdb else {
            panic!("expected a 16-byte CDB");
        };
        assert_eq!(cdb[0], 0x85); // ATA PASS-THROUGH (16)
        assert_eq!(cdb[1], 0x6 << 1 | 1); // DMA protocol, EXTEND bit set
        assert_eq!(cdb[2], 0x16); // byte2: t_dir=1, byte_block=1, t_length=0b10
        assert_eq!(cdb[3], 0x12); // feature high
        assert_eq!(cdb[4], 0x34); // feature low
        assert_eq!(cdb[5], 0x56); // count high
        assert_eq!(cdb[6], 0x78); // count low
        assert_eq!(cdb[7], 0x03); // lba(31:24)
        assert_eq!(cdb[8], 0x06); // lba(7:0)
        assert_eq!(cdb[9], 0x02); // lba(39:32)
        assert_eq!(cdb[10], 0x05); // lba(15:8)
        assert_eq!(cdb[11], 0x01); // lba(47:40)
        assert_eq!(cdb[12], 0x04); // lba(23:16)
        assert_eq!(cdb[13], 0xAA); // device
        assert_eq!(cdb[14], AtaCmd::ReadLogDmaExt as u8);
        assert_eq!(cdb[15], 0xBB); // control

        assert!(matches!(
            scsi.xfer,
            XferParameter::XferDirection(XferDirection::TargetToInitiator(XferLength::Sectors(1)))
        ));
    }

    #[test]
    fn build_encodes_ata_pass_through_32_cdb_and_xfer() {
        let ata_cmd = Ata::new()
            .feature(0x1234)
            .count(0x5678)
            .lba(0x0102_0304_0506)
            .control(0xBB)
            .icc(0xCC)
            .aux(0x1122_3344)
            .device(0xAA)
            .command(AtaCmd::ReadLogDmaExt)
            .protocol(AtaProtocol::Dma {
                direction: XferDirection::TargetToInitiator(XferLength::Sectors(1)),
            });

        let scsi = AtaPt::new(ata_cmd).cdb32();

        let Cdb::Cdb32(cdb) = scsi.cdb else {
            panic!("expected a 32-byte CDB");
        };
        assert_eq!(
            cdb,
            [
                0x7F, 0xBB, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x1F, 0xF0, 0x0D, 0x16,
                0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x12, 0x34, 0x56, 0x78,
                0xAA, AtaCmd::ReadLogDmaExt as u8, 0x00, 0xCC, 0x11, 0x22, 0x33, 0x44,
            ]
        );
        assert!(matches!(
            scsi.xfer,
            XferParameter::XferDirection(XferDirection::TargetToInitiator(XferLength::Sectors(1)))
        ));
    }

    #[test]
    fn read16_build_encodes_read_16_cdb_and_xfer() {
        let scsi = Read16::new()
            .lba(0x0102_0304_0506_0708)
            .transfer_length(0x1000)
            .fua(true)
            .control(0x55)
            .build();

        let Cdb::Cdb16(cdb) = scsi.cdb else {
            panic!("expected a 16-byte CDB");
        };
        assert_eq!(cdb[0], 0x88); // READ (16)
        assert_eq!(cdb[1], 0b100); // FUA
        assert_eq!(&cdb[2..10], &0x0102_0304_0506_0708u64.to_be_bytes());
        assert_eq!(&cdb[10..14], &0x1000u32.to_be_bytes());
        assert_eq!(cdb[14], 0);
        assert_eq!(cdb[15], 0x55);

        assert!(matches!(
            scsi.xfer,
            XferParameter::XferDirection(XferDirection::TargetToInitiator(XferLength::Sectors(
                0x1000
            )))
        ));
    }
}
