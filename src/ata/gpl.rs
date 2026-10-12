//! General Purpose Logging (GPL) command construction.

use super::{Command, CommandCode};
use crate::blk_cmd::XferLength;

pub struct Gpl {
    feature: u16,
    page_number: u16,
    page_count: u16,
    address: u8,
}

impl Gpl {
    /// Creates a builder; selecting a log command finishes it as an ATA command.
    pub fn builder() -> GplBuilder {
        GplBuilder::new()
    }

    pub fn new() -> Self {
        Self {
            feature: 0,
            page_number: 0,
            page_count: 1,
            address: 0,
        }
    }

    fn lba(&self) -> u64 {
        (u64::from(self.page_number >> 8) << 32)
            | (u64::from(self.page_number as u8) << 8)
            | u64::from(self.address)
    }
}

/// Builds a GPL command, finishing when a read or write method is selected.
#[derive(Default)]
pub struct GplBuilder {
    gpl: Gpl,
}

impl GplBuilder {
    pub fn new() -> Self {
        Self { gpl: Gpl::new() }
    }

    /// Sets the log-specific feature for reads; writes reserve this field.
    pub fn feature(mut self, feature: u16) -> Self {
        self.gpl.feature = feature;
        self
    }

    pub fn page_number(mut self, page_number: u16) -> Self {
        self.gpl.page_number = page_number;
        self
    }

    pub fn page_count(mut self, page_count: u16) -> Self {
        self.gpl.page_count = page_count;
        self
    }

    pub fn address(mut self, address: u8) -> Self {
        self.gpl.address = address;
        self
    }

    pub fn read_log_dma_ext(self) -> Command {
        Command::builder()
            .feature(self.gpl.feature)
            .count(self.gpl.page_count)
            .lba(self.gpl.lba())
            .command(CommandCode::ReadLogDmaExt)
            .dma_data_in(XferLength::Pages(u32::from(self.gpl.page_count)))
    }

    pub fn write_log_dma_ext(self) -> Command {
        Command::builder()
            .count(self.gpl.page_count)
            .lba(self.gpl.lba())
            .command(CommandCode::WriteLogDmaExt)
            .dma_data_out(XferLength::Pages(u32::from(self.gpl.page_count)))
    }

    pub fn receive_fpdma_queued(self) -> Command {
        Command::builder()
            .feature(self.gpl.page_count)
            .count(0x01 << 8) // READ LOG DMA EXT SUBCOMMAND
            .lba(self.gpl.lba())
            .aux(u32::from(self.gpl.feature))
            .device(0x40)
            .command(CommandCode::ReceiveFpdmaQueued)
            .ncq_data_in(XferLength::Pages(u32::from(self.gpl.page_count)))
    }

    pub fn send_fpdma_queued(self) -> Command {
        Command::builder()
            .feature(self.gpl.page_count)
            .count(0x02 << 8) // WRITE LOG DMA EXT SUBCOMMAND
            .lba(self.gpl.lba())
            .device(0x40)
            .command(CommandCode::SendFpdmaQueued)
            .ncq_data_out(XferLength::Pages(u32::from(self.gpl.page_count)))
    }
}

impl Default for Gpl {
    fn default() -> Self {
        Self::new()
    }
}
