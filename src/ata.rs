use crate::sg_io::{XferDirection, XferLength};
#[repr(u8)]
enum AtaProtocol {
    NonData = 0x03,
    PioDataIn = 0x04,
    PioDataOut = 0x05,
    Dma = 0x06,
    Ncq = 0x0C,
}
pub struct XferParam {
    pub direction: XferDirection,
    pub length: XferLength,
    pub protocol: AtaProtocol,
}

pub trait Ata {
    fn command(&self) -> u8 {
        0
    }
    fn feature(&self) -> u16 {
        0
    }
    fn count(&self) -> u16 {
        0
    }
    fn lba(&self) -> u64 {
        0
    }
    fn icc(&self) -> u8 {
        0
    }
    fn aux(&self) -> u32 {
        0
    }
    fn device(&self) -> u8 {
        0
    }
    fn control(&self) -> u8 {
        0
    }
    fn xfer_length(&self) -> XferParam;
    fn protocol(&self) -> u8 {
        self.xfer_length().protocol as u8
    }
}
pub fn fis(ata: &impl Ata) -> [u8; 20] {
    let mut fis = [0u8; 20];
    fis[0] = 0x27; // FIS Type: Register - Host to Device
    fis[1] = 0x80; // C bit set
    fis[2] = ata.command();
    fis[3] = ata.feature() as u8;
    fis[4] = ata.lba() as u8;
    fis[5] = (ata.lba() >> 8) as u8;
    fis[6] = (ata.lba() >> 16) as u8;
    fis[7] = ata.device();
    fis[8] = (ata.lba() >> 24) as u8;
    fis[9] = (ata.lba() >> 32) as u8;
    fis[10] = (ata.lba() >> 40) as u8;
    fis[11] = (ata.feature() >> 8) as u8;
    fis[12] = ata.count() as u8;
    fis[13] = (ata.count() >> 8) as u8;
    fis[14] = ata.icc();
    fis[15] = ata.control();
    fis[16] = ata.aux() as u8;
    fis[17] = (ata.aux() >> 8) as u8;
    fis[18] = (ata.aux() >> 16) as u8;
    fis[19] = (ata.aux() >> 24) as u8;
    fis
}
macro_rules! define_gpl {
    ($name:ident, $cmd:expr, $dir:expr, $protocol:expr) => {
        pub struct $name {
            log_page_count: u16,
            log_address: u8,
            page_number: u16,
        }
        impl $name {
            pub fn new(log_page_count: u16, log_address: u8, page_number: u16) -> Self {
                Self {
                    log_page_count,
                    log_address,
                    page_number,
                }
            }
            pub fn log_page_count(&self) -> u16 {
                self.log_page_count
            }
            pub fn log_address(&self) -> u8 {
                self.log_address
            }
            pub fn page_number(&self) -> u16 {
                self.page_number
            }
            fn page_number_hi(&self) -> u8 {
                (self.page_number >> 8) as u8
            }
            fn page_number_lo(&self) -> u8 {
                (self.page_number & 0xFF) as u8
            }
        }
        impl Ata for $name {
            fn command(&self) -> u8 {
                $cmd
            }
            fn count(&self) -> u16 {
                self.log_page_count
            }
            fn lba(&self) -> u64 {
                ((self.page_number_hi() as u64) << 32)
                    | ((self.page_number_lo() as u64) << 8)
                    | (self.log_address as u64)
            }
            fn xfer_length(&self) -> XferParam {
                XferParam {
                    direction: $dir,
                    length: XferLength::Pages(self.log_page_count as u32),
                    protocol: $protocol,
                }
            }
        }
    };
}
define_gpl!(
    ReadLogExt,
    0x2F,
    XferDirection::TargetToInitiator,
    AtaProtocol::PioDataIn
);
define_gpl!(
    WriteLogExt,
    0x3F,
    XferDirection::InitiatorToTarget,
    AtaProtocol::PioDataOut
);
define_gpl!(
    ReadLogDmaExt,
    0x47,
    XferDirection::TargetToInitiator,
    AtaProtocol::Dma
);
define_gpl!(
    WriteLogDmaExt,
    0x57,
    XferDirection::InitiatorToTarget,
    AtaProtocol::Dma
);

macro_rules! define_rw {
    ($name:ident, $cmd:expr, $dir:expr, $protocol:expr) => {
        pub struct $name {
            count: u16,
            lba: u64,
        }
        impl $name {
            pub fn new(count: u16, lba: u64) -> Self {
                Self {
                    count: count & 0xFF,
                    lba: lba & ((1 << 28) - 1),
                }
            }
        }
        impl Ata for $name {
            fn command(&self) -> u8 {
                $cmd
            }
            fn count(&self) -> u16 {
                self.count
            }
            fn lba(&self) -> u64 {
                self.lba
            }
            fn xfer_length(&self) -> XferParam {
                XferParam {
                    direction: $dir,
                    length: XferLength::Sectors(self.count as u32),
                    protocol: $protocol,
                }
            }
        }
    };
}
define_rw!(
    ReadSectors,
    0x20,
    XferDirection::TargetToInitiator,
    AtaProtocol::PioDataIn
);
define_rw!(
    WriteSectors,
    0x30,
    XferDirection::InitiatorToTarget,
    AtaProtocol::PioDataOut
);
define_rw!(
    ReadDma,
    0xC8,
    XferDirection::TargetToInitiator,
    AtaProtocol::Dma
);
define_rw!(
    WriteDma,
    0xCA,
    XferDirection::InitiatorToTarget,
    AtaProtocol::Dma
);
macro_rules! define_rw_ext {
    ($name:ident, $cmd:expr, $dir:expr, $protocol:expr) => {
        pub struct $name {
            count: u16,
            lba: u64,
        }
        impl $name {
            pub fn new(count: u16, lba: u64) -> Self {
                Self {
                    count: count & 0xFFFF,
                    lba: lba & ((1 << 48) - 1),
                }
            }
        }
        impl Ata for $name {
            fn command(&self) -> u8 {
                $cmd
            }
            fn count(&self) -> u16 {
                self.count
            }
            fn lba(&self) -> u64 {
                self.lba
            }
            fn device(&self) -> u8 {
                1 << 6
            }
            fn xfer_length(&self) -> XferParam {
                XferParam {
                    direction: $dir,
                    length: XferLength::Sectors(self.count as u32),
                    protocol: $protocol,
                }
            }
        }
    };
}
define_rw_ext!(
    ReadSectorsExt,
    0x24,
    XferDirection::TargetToInitiator,
    AtaProtocol::PioDataIn
);
define_rw_ext!(
    WriteSectorsExt,
    0x34,
    XferDirection::InitiatorToTarget,
    AtaProtocol::PioDataOut
);
macro_rules! define_rw_dma_ext {
    ($name:ident, $cmd:expr, $dir:expr, $protocol:expr) => {
        pub struct $name {
            feature: u16,
            count: u16,
            lba: u64,
            aux: u32,
        }
        impl $name {
            pub fn new(cdl: u8, count: u16, lba: u64, hybrid: u8) -> Self {
                Self {
                    feature: cdl as u16,
                    count: count & 0xFFFF,
                    lba: lba & ((1 << 48) - 1),
                    aux: (hybrid as u32) << 16,
                }
            }
        }
        impl Ata for $name {
            fn command(&self) -> u8 {
                $cmd
            }
            fn count(&self) -> u16 {
                self.count
            }
            fn lba(&self) -> u64 {
                self.lba
            }
            fn aux(&self) -> u32 {
                self.aux
            }
            fn device(&self) -> u8 {
                1 << 6
            }
            fn xfer_length(&self) -> XferParam {
                XferParam {
                    direction: $dir,
                    length: XferLength::Sectors(self.count as u32),
                    protocol: $protocol,
                }
            }
        }
    };
}
define_rw_dma_ext!(
    ReadDmaExt,
    0x25,
    XferDirection::TargetToInitiator,
    AtaProtocol::Dma
);
define_rw_dma_ext!(
    WriteDmaExt,
    0x35,
    XferDirection::InitiatorToTarget,
    AtaProtocol::Dma
);
