use crate::scsi::{Cdb, Scsi};
use std::fmt;
use std::io;

#[derive(Copy, Clone)]
pub enum XferDirection {
    TargetToInitiator(XferLength),
    InitiatorToTarget(XferLength),
}
#[derive(Copy, Clone)]
pub enum XferParameter {
    XferDirection(XferDirection),
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
    UnexpectedResidual {
        requested: u32,
        residual: i32,
    },
    ScsiError {
        status: u8,
        host_status: u16,
        driver_status: u16,
        sense: [u8; 32],
    },
}

fn format_sense(f: &mut fmt::Formatter<'_>, sense: &[u8; 32]) -> fmt::Result {
    let highlighted_indices: &[usize] = match sense[0] & 0x7f {
        0x70 | 0x71 => &[2, 12, 13],
        0x72 | 0x73 => &[1, 2, 3],
        _ => &[],
    };

    for (index, byte) in sense.iter().enumerate() {
        if index != 0 {
            write!(f, " ")?;
        }

        if highlighted_indices.contains(&index) {
            write!(f, "[{byte:02X}]")?;
        } else {
            write!(f, "{byte:02X}")?;
        }
    }

    Ok(())
}

impl fmt::Display for SgIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            SgIoErrorKind::IoctlFailed(error) => write!(f, "SG_IO ioctl failed: {error}"),
            SgIoErrorKind::InvalidTransferLength => write!(f, "invalid transfer length"),
            SgIoErrorKind::TransferTooLarge { requested, maximum } => {
                write!(
                    f,
                    "transfer too large: requested={requested} bytes, maximum={maximum} bytes"
                )
            }
            SgIoErrorKind::TransferLengthMismatch { expected, actual } => {
                write!(
                    f,
                    "transfer length mismatch: expected={expected} bytes, actual={actual} bytes"
                )
            }
            SgIoErrorKind::UnexpectedResidual { requested, residual } => {
                write!(
                    f,
                    "incomplete or invalid transfer: requested={requested} bytes, residual={residual} bytes"
                )
            }
            SgIoErrorKind::ScsiError {
                status,
                host_status,
                driver_status,
                sense,
            } => {
                write!(
                    f,
                    "SCSI command failed: status=0x{status:02X}, host_status=0x{host_status:04X}, \
                     driver_status=0x{driver_status:04X}, sense="
                )?;
                format_sense(f, sense)
            }
        }
    }
}

impl std::error::Error for SgIoError {}

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

fn check_completion(hdr: &SgIoHdr, sense: [u8; 32]) -> Result<(), SgIoError> {
    if hdr.status != 0 || hdr.host_status != 0 || hdr.driver_status != 0 {
        return Err(SgIoError {
            kind: SgIoErrorKind::ScsiError {
                status: hdr.status,
                host_status: hdr.host_status,
                driver_status: hdr.driver_status,
                sense,
            },
        });
    }

    // execute() promises the complete requested transfer. Keep the signed
    // residual so overruns and malformed completion values are also rejected.
    if hdr.resid != 0 {
        return Err(SgIoError {
            kind: SgIoErrorKind::UnexpectedResidual {
                requested: hdr.dxfer_len,
                residual: hdr.resid,
            },
        });
    }

    Ok(())
}

impl Cdb {
    fn as_slice(&self) -> &[u8] {
        match self {
            Cdb::Cdb6(b) => &b[..],
            Cdb::Cdb10(b) => &b[..],
            Cdb::Cdb12(b) => &b[..],
            Cdb::Cdb16(b) => &b[..],
            Cdb::Cdb32(b) => &b[..],
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

    fn transfer_len_bytes(&self, xfer: XferParameter) -> Result<usize, SgIoError> {
        let length = match xfer {
            XferParameter::XferDirection(XferDirection::TargetToInitiator(length))
            | XferParameter::XferDirection(XferDirection::InitiatorToTarget(length)) => {
                if matches!(
                    length,
                    XferLength::Sectors(0) | XferLength::Pages(0) | XferLength::Bytes(0)
                ) {
                    return Err(SgIoError {
                        kind: SgIoErrorKind::InvalidTransferLength,
                    });
                }
                length
            }
            XferParameter::NoDataTransfer => return Ok(0),
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

    /// Executes a command and requires the entire requested buffer to transfer.
    /// A non-zero residual is returned as an error; partially transferred data
    /// may still be present in `buf` when execution fails.
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
            XferParameter::XferDirection(XferDirection::TargetToInitiator(_)) => SG_DXFER_FROM_DEV,
            XferParameter::XferDirection(XferDirection::InitiatorToTarget(_)) => SG_DXFER_TO_DEV,
            XferParameter::NoDataTransfer => SG_DXFER_NONE,
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

        check_completion(&hdr, sense_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::{Device, SgIoError, SgIoErrorKind, XferDirection, XferLength, XferParameter};
    use crate::scsi::{Cdb, Scsi};

    fn completion_header(requested: u32, residual: i32) -> super::SgIoHdr {
        super::SgIoHdr {
            interface_id: b'S' as i32,
            dxfer_direction: super::SG_DXFER_FROM_DEV,
            cmd_len: 16,
            mx_sb_len: 32,
            iovec_count: 0,
            dxfer_len: requested,
            dxferp: std::ptr::null_mut(),
            cmdp: std::ptr::null(),
            sbp: std::ptr::null_mut(),
            timeout: 1000,
            flags: 0,
            pack_id: 0,
            usr_ptr: std::ptr::null_mut(),
            status: 0,
            masked_status: 0,
            msg_status: 0,
            sb_len_wr: 0,
            host_status: 0,
            driver_status: 0,
            resid: residual,
            duration: 0,
            info: 0,
        }
    }

    #[test]
    fn exact_completion_accepts_full_and_no_data_transfers() {
        for requested in [0, 512, 4096] {
            assert!(super::check_completion(&completion_header(requested, 0), [0; 32]).is_ok());
        }
    }

    #[test]
    fn good_status_does_not_hide_short_or_invalid_transfers() {
        for residual in [1, 512, 4096, 4097, -1, i32::MIN, i32::MAX] {
            let error = super::check_completion(&completion_header(4096, residual), [0; 32])
                .unwrap_err();
            assert!(matches!(error.kind, SgIoErrorKind::UnexpectedResidual {
                requested: 4096, residual: value,
            } if value == residual));
            assert!(error.to_string().contains(&format!("residual={residual}")));
        }
    }

    #[test]
    fn completion_preserves_scsi_and_transport_errors_before_residuals() {
        for (status, host_status, driver_status) in [(2, 0, 0), (0, 1, 0), (0, 0, 8)] {
            let mut hdr = completion_header(4096, 512);
            hdr.status = status;
            hdr.host_status = host_status;
            hdr.driver_status = driver_status;
            let mut sense = [0; 32];
            sense[0] = 0x72;
            sense[1] = 0x05;
            sense[2] = 0x24;
            let error = super::check_completion(&hdr, sense).unwrap_err();
            assert!(matches!(error.kind, SgIoErrorKind::ScsiError {
                status: s, host_status: h, driver_status: d, sense: actual_sense,
            } if s == status && h == host_status && d == driver_status && actual_sense == sense));
        }
    }

    fn read_command(sectors: u32) -> Scsi {
        Scsi {
            cdb: Cdb::Cdb16([0; 16]),
            xfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                XferLength::Sectors(sectors),
            )),
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
            xfer: XferParameter::NoDataTransfer,
        };

        assert!(device.allocate(&command).unwrap().is_empty());
    }

    #[test]
    fn zero_length_transfer_is_rejected() {
        let device = Device::new(-1, 512, 1);

        assert!(matches!(
            device.allocate(&read_command(0)),
            Err(error) if matches!(error.kind, SgIoErrorKind::InvalidTransferLength)
        ));
    }

    #[test]
    fn pages_and_bytes_lengths_are_converted_to_the_expected_byte_count() {
        let device = Device::new(-1, 512, 10);

        let pages = Scsi {
            cdb: Cdb::Cdb16([0; 16]),
            xfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                XferLength::Pages(2),
            )),
        };
        assert_eq!(device.allocate(&pages).unwrap().len(), 1024);

        let bytes = Scsi {
            cdb: Cdb::Cdb16([0; 16]),
            xfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                XferLength::Bytes(777),
            )),
        };
        assert_eq!(device.allocate(&bytes).unwrap().len(), 777);
    }

    #[test]
    fn display_highlights_key_asc_and_ascq_in_fixed_format_sense() {
        let mut sense = [0; 32];
        sense[0] = 0xf0;
        sense[2] = 0x05;
        sense[12] = 0x20;
        sense[13] = 0x00;

        let error = SgIoError {
            kind: SgIoErrorKind::ScsiError {
                status: 2,
                host_status: 0,
                driver_status: 0,
                sense,
            },
        };

        assert!(error
            .to_string()
            .contains("sense=F0 00 [05] 00 00 00 00 00 00 00 00 00 [20] [00]"));
    }

    #[test]
    fn display_highlights_key_asc_and_ascq_in_descriptor_format_sense() {
        let mut sense = [0; 32];
        sense[0] = 0x72;
        sense[1] = 0x06;
        sense[2] = 0x29;
        sense[3] = 0x00;

        let error = SgIoError {
            kind: SgIoErrorKind::ScsiError {
                status: 2,
                host_status: 0,
                driver_status: 0,
                sense,
            },
        };

        assert!(error
            .to_string()
            .contains("sense=72 [06] [29] [00]"));
    }
}

