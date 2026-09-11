use crate::scsi::{Cdb, Scsi};
use std::io;

#[derive(Copy, Clone)]
pub enum XferDirection {
    TargetToInitiator(XferLength),
    InitiatorToTarget(XferLength),
    NoDataTransfer,
}

#[derive(Copy, Clone)]
pub enum XferLength {
    Sectors(u32),
    Pages(u32),
    Bytes(usize),
}

pub struct Device {
    fd: i32,
    sector_size_bytes: u32,
    max_sectors_kbytes: u32,
}

#[derive(Debug)]
pub struct SgIoError {
    pub kind: SgIoErrorKind,
}

#[derive(Debug)]
pub enum SgIoErrorKind {
    IoctlFailed(io::Error),
    InvalidTransferLength,
    TransferTooLarge {
        requested: usize,
        maximum: usize,
    },
    TransferLengthMismatch {
        expected: usize,
        actual: usize,
    },
    ScsiError {
        status: u8,
        host_status: u16,
        driver_status: u16,
    },
}

#[repr(C)]
struct SgIoHdr {
    interface_id: i32,
    dxfer_direction: i32,
    cmd_len: u8,
    mx_sb_len: u8,
    iovec_count: u16,
    dxfer_len: u32,
    dxferp: *mut u8,
    cmdp: *const u8,
    sbp: *mut u8,
    timeout: u32,
    flags: u32,
    pack_id: i32,
    usr_ptr: *mut libc::c_void,
    status: u8,
    masked_status: u8,
    msg_status: u8,
    sb_len_wr: u8,
    host_status: u16,
    driver_status: u16,
    resid: i32,
    duration: u32,
    info: u32,
}

const SG_IO: libc::c_ulong = 0x2285;
const SG_DXFER_NONE: i32 = -1;
const SG_DXFER_TO_DEV: i32 = -2;
const SG_DXFER_FROM_DEV: i32 = -3;
const DEFAULT_TIMEOUT_MILLIS: u32 = 1200_000_000;

impl Cdb {
    fn as_slice(&self) -> &[u8] {
        match self {
            Cdb::Cdb6(b) => &b[..],
            Cdb::Cdb10(b) => &b[..],
            Cdb::Cdb12(b) => &b[..],
            Cdb::Cdb16(b) => &b[..],
        }
    }
}

impl Device {
    pub fn new(fd: i32, sector_size_bytes: u32, max_sectors_kbytes: u32) -> Self {
        Self {
            fd,
            sector_size_bytes,
            max_sectors_kbytes,
        }
    }

    fn transfer_len_bytes(&self, xfer: XferDirection) -> Result<usize, SgIoError> {
        let length = match xfer {
            XferDirection::TargetToInitiator(length) | XferDirection::InitiatorToTarget(length) => {
                if matches!(length, XferLength::Sectors(0) | XferLength::Pages(0) | XferLength::Bytes(0)) {
                    return Err(SgIoError {
                        kind: SgIoErrorKind::InvalidTransferLength,
                    });
                }
                length
            }
            XferDirection::NoDataTransfer => return Ok(0),
        };

        let requested = match length {
            XferLength::Sectors(sectors) => usize::try_from(sectors)
                .ok()
                .and_then(|sectors| sectors.checked_mul(self.sector_size_bytes as usize)),
            XferLength::Pages(pages) => usize::try_from(pages)
                .ok()
                .and_then(|pages| pages.checked_mul(512)),
            XferLength::Bytes(bytes) => Some(bytes),
        }
        .ok_or(SgIoError {
            kind: SgIoErrorKind::InvalidTransferLength,
        })?;

        let maximum = (self.max_sectors_kbytes as usize)
            .checked_mul(1024)
            .ok_or(SgIoError {
                kind: SgIoErrorKind::InvalidTransferLength,
            })?;

        if requested > maximum {
            return Err(SgIoError {
                kind: SgIoErrorKind::TransferTooLarge { requested, maximum },
            });
        }

        Ok(requested)
    }

    pub fn allocate(&self, cmd: &Scsi) -> Result<Vec<u8>, SgIoError> {
        Ok(vec![0; self.transfer_len_bytes(cmd.xfer)?])
    }

    pub fn execute(&self, cmd: &Scsi, buf: &mut [u8]) -> Result<(), SgIoError> {
        let cdb_slice = cmd.cdb.as_slice();
        let xfer = cmd.xfer;
        let expected_len = self.transfer_len_bytes(xfer)?;

        if buf.len() != expected_len {
            return Err(SgIoError {
                kind: SgIoErrorKind::TransferLengthMismatch {
                    expected: expected_len,
                    actual: buf.len(),
                },
            });
        }

        let dxfer_len = u32::try_from(buf.len()).map_err(|_| SgIoError {
            kind: SgIoErrorKind::InvalidTransferLength,
        })?;

        let dxfer_direction = match xfer {
            XferDirection::TargetToInitiator(_) => SG_DXFER_FROM_DEV,
            XferDirection::InitiatorToTarget(_) => SG_DXFER_TO_DEV,
            XferDirection::NoDataTransfer => SG_DXFER_NONE,
        };

        let mut sense_buffer = [0u8; 32];
        let mut hdr = SgIoHdr {
            interface_id: b'S' as i32,
            dxfer_direction,
            cmd_len: cdb_slice.len() as u8,
            mx_sb_len: sense_buffer.len() as u8,
            iovec_count: 0,
            dxfer_len,
            dxferp: buf.as_mut_ptr(),
            cmdp: cdb_slice.as_ptr(),
            sbp: sense_buffer.as_mut_ptr(),
            timeout: DEFAULT_TIMEOUT_MILLIS,
            flags: 0,
            pack_id: 0,
            usr_ptr: std::ptr::null_mut(),
            status: 0,
            masked_status: 0,
            msg_status: 0,
            sb_len_wr: 0,
            host_status: 0,
            driver_status: 0,
            resid: 0,
            duration: 0,
            info: 0,
        };

        let ret = unsafe { libc::ioctl(self.fd, SG_IO, &mut hdr) };
        if ret < 0 {
            return Err(SgIoError {
                kind: SgIoErrorKind::IoctlFailed(io::Error::last_os_error()),
            });
        }

        if hdr.status != 0 || hdr.host_status != 0 || hdr.driver_status != 0 {
            return Err(SgIoError {
                kind: SgIoErrorKind::ScsiError {
                    status: hdr.status,
                    host_status: hdr.host_status,
                    driver_status: hdr.driver_status,
                },
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Device, SgIoErrorKind, XferDirection, XferLength};
    use crate::scsi::{Cdb, Scsi};

    fn read_command(sectors: u32) -> Scsi {
        Scsi {
            cdb: Cdb::Cdb16([0; 16]),
            xfer: XferDirection::TargetToInitiator(XferLength::Sectors(sectors)),
        }
    }

    #[test]
    fn allocation_respects_transfer_length_and_device_limit() {
        let device = Device::new(-1, 512, 1);

        assert_eq!(device.allocate(&read_command(2)).unwrap().len(), 1024);
        assert!(matches!(
            device.allocate(&read_command(3)),
            Err(error) if matches!(error.kind, SgIoErrorKind::TransferTooLarge { .. })
        ));
    }

    #[test]
    fn no_data_command_uses_an_empty_buffer() {
        let device = Device::new(-1, 512, 1);
        let command = Scsi {
            cdb: Cdb::Cdb16([0; 16]),
            xfer: XferDirection::NoDataTransfer,
        };

        assert!(device.allocate(&command).unwrap().is_empty());
    }
}
