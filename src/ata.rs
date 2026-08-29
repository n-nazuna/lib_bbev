use crate::sg_io::{XferDirection, XferLength};
#[repr(u8)]
#[derive(Copy, Clone)]
pub(crate) enum AtaProtocol {
    NonData = 0x03,
    PioDataIn = 0x04,
    PioDataOut = 0x05,
    Dma = 0x06,
    Ncq = 0x0C,
}
#[derive(Copy, Clone)]
pub struct XferParam {
    pub direction: XferDirection,
    pub length: XferLength,
    pub(crate) protocol: AtaProtocol,
}

pub trait Ata {
    fn command(&self) -> u8;
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
trait GplTrait {
    fn feature(&self) -> u16;
    fn page_count(&self) -> u16;
    fn page_number(&self) -> u16;
    fn address(&self) -> u8;
}
pub struct Gpl {
    feature: u16,
    log_page_count: u16,
    page_number: u16,
    log_address: u8,
    command: u8,
    xfer_param: XferParam,
}
impl GplTrait for Gpl {
    fn feature(&self) -> u16 {
        self.feature
    }
    fn page_count(&self) -> u16 {
        self.log_page_count
    }
    fn page_number(&self) -> u16 {
        self.page_number
    }
    fn address(&self) -> u8 {
        self.log_address
    }
}
impl Gpl {
    fn page_number_hi(&self) -> u8 {
        (self.page_number >> 8) as u8
    }
    fn page_number_lo(&self) -> u8 {
        (self.page_number & 0xFF) as u8
    }
}
impl Ata for Gpl {
    fn command(&self) -> u8 {
        self.command
    }
    fn count(&self) -> u16 {
        self.page_count()
    }
    fn lba(&self) -> u64 {
        ((self.page_number_hi() as u64) << 32)
            | ((self.page_number_lo() as u64) << 8)
            | (self.address() as u64)
    }
    fn feature(&self) -> u16 {
        self.feature
    }
    fn xfer_length(&self) -> XferParam {
        XferParam {
            direction: self.xfer_param.direction,
            length: XferLength::Pages(self.page_count() as u32),
            protocol: self.xfer_param.protocol,
        }
    }
}
impl Gpl {
    pub fn page_count(mut self, count: u16) -> Self {
        self.log_page_count = count;
        self
    }
    pub fn page_number(mut self, number: u16) -> Self {
        self.page_number = number;
        self
    }
    pub fn address(mut self, address: u8) -> Self {
        self.log_address = address;
        self
    }
}
macro_rules! define_gpl {
    ($name:ident, $cmd:expr, $dir:expr, $protocol:expr) => {
        impl Gpl {
            pub fn $name() -> Self {
                Gpl {
                    feature: 0,
                    log_page_count: 0,
                    page_number: 0,
                    log_address: 0,
                    command: $cmd,
                    xfer_param: XferParam {
                        direction: $dir,
                        length: XferLength::Pages(0),
                        protocol: $protocol,
                    },
                }
            }
        }
    };
}

define_gpl!(
    read_log_ext,
    0x2F,
    XferDirection::TargetToInitiator,
    AtaProtocol::PioDataIn
);
define_gpl!(
    read_log_dma_ext,
    0x47,
    XferDirection::TargetToInitiator,
    AtaProtocol::Dma
);
define_gpl!(
    write_log_ext,
    0x3F,
    XferDirection::InitiatorToTarget,
    AtaProtocol::PioDataOut
);
define_gpl!(
    write_log_dma_ext,
    0x57,
    XferDirection::InitiatorToTarget,
    AtaProtocol::Dma
);

trait NcqTrait {
    fn feature(&self) -> u16;
    fn count(&self) -> u16;
    fn lba(&self) -> u64;
    fn aux(&self) -> u32;
}
struct Ncq {
    feature: u16,
    count: u16,
    lba: u64,
    aux: u32,
    command: u8,
    xfer_param: XferParam,
}
impl NcqTrait for Ncq {
    fn feature(&self) -> u16 {
        self.feature
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
}