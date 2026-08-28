use crate::sg_io::{XferDirection, XferLength, XferParam};
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
    fn aux(&self) -> u16 {
        0
    }
    fn device(&self) -> u8 {
        0
    }
    fn xfer_length(&self) -> XferParam;
}
macro_rules! define_gpl {
    ($name:ident, $cmd:expr, $dir:expr) => {
        pub struct $name {
            pub log_page_count: u16,
            pub log_address: u8,
            pub page_number: u16,
        }
        impl $name {
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
                }
            }
        }
    };
}
define_gpl!(ReadLogExt, 0x2F, XferDirection::TargetToInitiator);
define_gpl!(ReadLogDmaExt, 0x47, XferDirection::TargetToInitiator);
define_gpl!(WriteLogExt, 0x3F, XferDirection::InitiatorToTarget);
define_gpl!(WriteLogDmaExt, 0x57, XferDirection::InitiatorToTarget);