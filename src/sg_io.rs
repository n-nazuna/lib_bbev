pub enum XferDirection {
    TargetToInitiator,
    InitiatorToTarget,
    NoDataTransfer,
}
pub enum XferLength {
    Sectors(u32),
    Pages(u32),
    Bytes(usize),
    None,
}
pub struct XferParam {
    pub direction: XferDirection,
    pub length: XferLength,
}

struct device {
    hctl: [u8; 4],
    sector_size_bytes: u32,
    max_sectors_kbytes: u32,
}
