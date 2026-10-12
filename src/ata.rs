pub mod command_code;
pub mod gpl;
use crate::blk_cmd::{XferDirection, XferLength};
pub use command_code::CommandCode;
pub enum AtaProtocol {
    NonData,
    Pio { direction: XferDirection },
    Dma { direction: XferDirection },
    NcqNonData,
    Ncq { direction: XferDirection },
}

pub struct Command {
    pub feature: u16,
    pub count: u16,
    pub lba: u64,
    pub control: u8,
    pub icc: u8,
    pub aux: u32,
    pub device: u8,
    pub command: command_code::CommandCode,
    pub protocol: AtaProtocol,
}
impl Command {
    /// Creates a builder with default register values; selecting a protocol finishes it.
    pub fn builder() -> CommandBuilder {
        CommandBuilder::new()
    }

    pub fn new() -> Self {
        Self {
            feature: 0,
            count: 0,
            lba: 0,
            control: 0,
            icc: 0,
            aux: 0,
            device: 0,
            command: CommandCode::Nop,
            protocol: AtaProtocol::NonData,
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
impl Default for Command {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds an ATA command, finishing when an execution protocol is selected.
#[derive(Default)]
pub struct CommandBuilder {
    ata: Command,
}

impl CommandBuilder {
    pub fn new() -> Self {
        Self {
            ata: Command::new(),
        }
    }

    pub fn feature(mut self, feature: u16) -> Self {
        self.ata.feature = feature;
        self
    }

    pub fn count(mut self, count: u16) -> Self {
        self.ata.count = count;
        self
    }

    pub fn lba(mut self, lba: u64) -> Self {
        self.ata.lba = lba;
        self
    }

    pub fn control(mut self, control: u8) -> Self {
        self.ata.control = control;
        self
    }

    pub fn icc(mut self, icc: u8) -> Self {
        self.ata.icc = icc;
        self
    }

    pub fn aux(mut self, aux: u32) -> Self {
        self.ata.aux = aux;
        self
    }

    pub fn device(mut self, device: u8) -> Self {
        self.ata.device = device;
        self
    }

    pub fn command(mut self, command: CommandCode) -> Self {
        self.ata.command = command;
        self
    }

    fn build(mut self, protocol: AtaProtocol) -> Command {
        self.ata.protocol = protocol;
        self.ata
    }

    pub fn nondata(self) -> Command {
        self.build(AtaProtocol::NonData)
    }

    pub fn pio_data_in(self, len: XferLength) -> Command {
        self.build(AtaProtocol::Pio {
            direction: XferDirection::TargetToInitiator(len),
        })
    }

    pub fn pio_data_out(self, len: XferLength) -> Command {
        self.build(AtaProtocol::Pio {
            direction: XferDirection::InitiatorToTarget(len),
        })
    }

    pub fn dma_data_in(self, len: XferLength) -> Command {
        self.build(AtaProtocol::Dma {
            direction: XferDirection::TargetToInitiator(len),
        })
    }

    pub fn dma_data_out(self, len: XferLength) -> Command {
        self.build(AtaProtocol::Dma {
            direction: XferDirection::InitiatorToTarget(len),
        })
    }

    pub fn ncq_nondata(self) -> Command {
        self.build(AtaProtocol::NcqNonData)
    }

    pub fn ncq_data_in(self, len: XferLength) -> Command {
        self.build(AtaProtocol::Ncq {
            direction: XferDirection::TargetToInitiator(len),
        })
    }

    pub fn ncq_data_out(self, len: XferLength) -> Command {
        self.build(AtaProtocol::Ncq {
            direction: XferDirection::InitiatorToTarget(len),
        })
    }
}
