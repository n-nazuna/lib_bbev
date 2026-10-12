//! Transport-independent transfer parameters shared by ATA, SCSI, and I/O backends.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum XferParameter {
    XferDirection(XferDirection),
    NoDataTransfer,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum XferDirection {
    TargetToInitiator(XferLength),
    InitiatorToTarget(XferLength),
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum XferLength {
    /// Logical sectors, converted using the device's sector size.
    Sectors(u32),
    /// Fixed-size 512-byte pages.
    Pages(u32),
    Bytes(usize),
}
