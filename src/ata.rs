use crate::sg_io::{XferDirection, XferLength};

pub enum AtaProtocol {
    NonData,
    Pio(XferLength),
    Dma(XferLength),
    NcqNonData,
    Ncq(XferLength),
}
impl AtaProtocol {
    pub fn length(&self) -> XferLength {
        match self {
            AtaProtocol::NonData | AtaProtocol::NcqNonData => XferLength::None,
            AtaProtocol::Pio(len) | AtaProtocol::Dma(len) | AtaProtocol::Ncq(len) => *len,
        }
    }
    // ATA PASS-THROUGH CDB PROTOCOL field values (SAT-3)
    pub fn code(&self, direction: XferDirection) -> u8 {
        match self {
            AtaProtocol::NonData => 0x3,
            AtaProtocol::Pio(_) => match direction {
                XferDirection::TargetToInitiator => 0x4,
                _ => 0x5,
            },
            AtaProtocol::Dma(_) => 0x6,
            AtaProtocol::NcqNonData | AtaProtocol::Ncq(_) => 0xC,
        }
    }
}
pub struct XferParam {
    pub direction: XferDirection,
    pub protocol: AtaProtocol,
}
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum AtaCmd {
    Nop = 0x00,
    ReadLogExt = 0x2F,
    ReadLogDmaExt = 0x47,
    WriteLogExt = 0x3F,
    WriteLogDmaExt = 0x57,
    ReadDmaExt = 0x25,
    WriteDmaExt = 0x35,
}
pub struct Ata {
    pub(crate) feature: u16,
    pub(crate) count: u16,
    pub(crate) lba: u64,
    pub(crate) control: u8,
    pub(crate) icc: u8,
    pub(crate) aux: u32,
    pub(crate) device: u8,
    pub(crate) command: AtaCmd,
    pub(crate) xfer_param: XferParam,
}
impl Ata {
    pub fn new() -> Self {
        Self {
            feature: 0,
            count: 0,
            lba: 0,
            control: 0,
            icc: 0,
            aux: 0,
            device: 0,
            command: AtaCmd::Nop,
            xfer_param: XferParam {
                direction: XferDirection::TargetToInitiator,
                protocol: AtaProtocol::NonData,
            },
        }
    }
    pub fn fis(&self) -> [u8; 20] {
        let mut fis = [0u8; 20];
        fis[0] = 0x27; // FIS Type: Register - Host to Device
        fis[1] = 0x80; // C bit set
        fis[2] = self.command as u8;
        fis[3] = self.feature as u8;
        fis[4] = self.lba as u8;
        fis[5] = (self.lba >> 8) as u8;
        fis[6] = (self.lba >> 16) as u8;
        fis[7] = self.device;
        fis[8] = (self.lba >> 24) as u8;
        fis[9] = (self.lba >> 32) as u8;
        fis[10] = (self.lba >> 40) as u8;
        fis[11] = (self.feature >> 8) as u8;
        fis[12] = self.count as u8;
        fis[13] = (self.count >> 8) as u8;
        fis[14] = self.icc;
        fis[15] = self.control;
        fis[16] = self.aux as u8;
        fis[17] = (self.aux >> 8) as u8;
        fis[18] = (self.aux >> 16) as u8;
        fis[19] = (self.aux >> 24) as u8;
        fis
    }
}
pub struct Gpl {
    feature: u16,
    page_number: u16,
    page_count: u16,
    address: u64,
    command: AtaCmd,
    xfer_param: XferParam,
}
impl Gpl {
    pub fn read_log_dma_ext() -> Self {
        Gpl {
            feature: 0,
            page_number: 0,
            page_count: 0,
            address: 0,
            command: AtaCmd::ReadLogDmaExt,
            xfer_param: XferParam {
                direction: XferDirection::TargetToInitiator,
                protocol: AtaProtocol::Dma(XferLength::Pages(0)),
            },
        }
    }
    pub fn write_log_dma_ext() -> Self {
        Gpl {
            feature: 0,
            page_number: 0,
            page_count: 0,
            address: 0,
            command: AtaCmd::WriteLogDmaExt,
            xfer_param: XferParam {
                direction: XferDirection::InitiatorToTarget,
                protocol: AtaProtocol::Dma(XferLength::Pages(0)),
            },
        }
    }
    pub fn page_number(mut self, page_number: u16) -> Self {
        self.page_number = page_number;
        self
    }
    fn page_number_hi(&self) -> u8 {
        (self.page_number >> 8) as u8
    }
    fn page_number_lo(&self) -> u8 {
        self.page_number as u8
    }
    fn lba(&self) -> u64 {
        ((self.page_number_hi() as u64) << 32)
            | ((self.page_number_lo() as u64) << 8)
            | (self.address as u64)
    }
    pub fn page_count(mut self, page_count: u16) -> Self {
        self.page_count = page_count;
        self
    }
    pub fn address(mut self, address: u64) -> Self {
        self.address = address;
        self
    }
    pub fn build(self) -> Ata {
        Ata {
            feature: 0,
            count: self.page_count,
            lba: self.lba(),
            control: 0,
            icc: 0,
            aux: 0,
            device: 0,
            command: self.command,
            xfer_param: XferParam {
                direction: self.xfer_param.direction,
                protocol: AtaProtocol::Dma(XferLength::Pages(self.page_count as u32)),
            },
        }
    }
}