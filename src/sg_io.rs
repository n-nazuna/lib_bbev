use crate::blk_cmd::{XferDirection, XferLength, XferParameter};
use crate::scsi::{Command, Response, SenseData, Status};
use std::fmt;
use std::io;

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
    TransportError {
        status: u8,
        host_status: u16,
        driver_status: u16,
        sense: Vec<u8>,
    },
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
            SgIoErrorKind::UnexpectedResidual {
                requested,
                residual,
            } => {
                write!(
                    f,
                    "incomplete or invalid transfer: requested={requested} bytes, residual={residual} bytes"
                )
            }
            SgIoErrorKind::TransportError {
                status,
                host_status,
                driver_status,
                sense,
            } => {
                write!(
                    f,
                    "SG_IO transport failed: status=0x{status:02X}, host_status=0x{host_status:04X}, \
                     driver_status=0x{driver_status:04X}, sense={sense:02X?}"
                )
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
const DEFAULT_TIMEOUT_MILLIS: u32 = 1_200_000_000;
// SG reports DRIVER_SENSE for CHECK CONDITION even when transport succeeded.
const DRIVER_SENSE: u16 = 0x08;

fn check_completion(hdr: &SgIoHdr, sense: &[u8]) -> Result<Response, SgIoError> {
    let sense_len = usize::from(hdr.sb_len_wr).min(sense.len());
    let sense = &sense[..sense_len];
    if hdr.host_status != 0
        || (hdr.driver_status != 0 && !(hdr.status == 0x02 && hdr.driver_status == DRIVER_SENSE))
    {
        return Err(SgIoError {
            kind: SgIoErrorKind::TransportError {
                status: hdr.status,
                host_status: hdr.host_status,
                driver_status: hdr.driver_status,
                sense: sense.to_vec(),
            },
        });
    }

    // A successful command must transfer the whole buffer. A failed command
    // may transfer only part of it; return its SCSI status and sense to the caller.
    if hdr.resid < 0 || hdr.resid as u32 > hdr.dxfer_len || (hdr.status == 0 && hdr.resid != 0) {
        return Err(SgIoError {
            kind: SgIoErrorKind::UnexpectedResidual {
                requested: hdr.dxfer_len,
                residual: hdr.resid,
            },
        });
    }

    Ok(Response {
        status: match hdr.status {
            0x00 => Status::Good,
            0x02 => Status::CheckCondition,
            0x08 => Status::Busy,
            0x18 => Status::ReservationConflict,
            status => Status::Other(status),
        },
        sense_data: SenseData::from_bytes(sense),
    })
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

    pub fn allocate(&self, cmd: &Command) -> Result<Vec<u8>, SgIoError> {
        Ok(vec![0; self.transfer_len_bytes(cmd.xfer)?])
    }
    /// GOOD requires a complete transfer. On other SCSI statuses or an error,
    /// partially transferred data may still be present in `buf`.
    pub fn execute(&self, cmd: &Command, buf: &mut [u8]) -> Result<Response, SgIoError> {
        let cdb_slice = cmd.cdb.as_bytes();
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

        check_completion(&hdr, &sense_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::{Device, SgIoErrorKind, SgIoHdr, check_completion};
    use crate::blk_cmd::{XferDirection, XferLength, XferParameter};
    use crate::scsi::{Cdb, Command, SenseData, Status};

    fn read_command(sectors: u32) -> Command {
        Command {
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
        let command = Command {
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

        let pages = Command {
            cdb: Cdb::Cdb16([0; 16]),
            xfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                XferLength::Pages(2),
            )),
        };
        assert_eq!(device.allocate(&pages).unwrap().len(), 1024);

        let bytes = Command {
            cdb: Cdb::Cdb16([0; 16]),
            xfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                XferLength::Bytes(777),
            )),
        };
        assert_eq!(device.allocate(&bytes).unwrap().len(), 777);
    }

    fn completion_header(status: u8) -> SgIoHdr {
        SgIoHdr {
            interface_id: b'S' as i32,
            dxfer_direction: super::SG_DXFER_FROM_DEV,
            cmd_len: 16,
            mx_sb_len: 32,
            iovec_count: 0,
            dxfer_len: 512,
            dxferp: std::ptr::null_mut(),
            cmdp: std::ptr::null(),
            sbp: std::ptr::null_mut(),
            timeout: super::DEFAULT_TIMEOUT_MILLIS,
            flags: 0,
            pack_id: 0,
            usr_ptr: std::ptr::null_mut(),
            status,
            masked_status: 0,
            msg_status: 0,
            sb_len_wr: 0,
            host_status: 0,
            driver_status: 0,
            resid: 0,
            duration: 0,
            info: 0,
        }
    }

    // 入力: GOOD / BUSY / RESERVATION CONFLICT / 未定義status、host/driverエラーなし。
    // 期待値: いずれもOk(Response)、statusを保持、senseなし。
    #[test]
    fn command_status_is_returned_as_a_response() {
        for (status, expected) in [
            (0x00, Status::Good),
            (0x08, Status::Busy),
            (0x18, Status::ReservationConflict),
            (0x28, Status::Other(0x28)),
        ] {
            let response = check_completion(&completion_header(status), &[0; 32]).unwrap();
            assert_eq!(response.status, expected, "SCSI STATUS");
            assert!(response.sense_data.is_none(), "SENSE DATA");
        }
    }

    // 入力: CHECK CONDITION、DRIVER_SENSE、残量512、22バイトのATA descriptor sense。
    // 期待値: 送信エラーにせずResponseを返し、SAT経由でSTATUS=51/ERROR=04を取得。
    #[test]
    fn check_condition_with_driver_sense_reaches_sat() {
        let mut hdr = completion_header(0x02);
        hdr.driver_status = super::DRIVER_SENSE;
        hdr.resid = 512;
        hdr.sb_len_wr = 22;
        let sense = [
            0x72, 0x0b, 0, 0, 0, 0, 0, 14, 0x09, 0x0c, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0x40, 0x51,
        ];
        let response = check_completion(&hdr, &sense).unwrap();
        assert_eq!(response.status, Status::CheckCondition, "SCSI STATUS");
        let sat = crate::scsi::sat::AtaPt::new(crate::ata::Command::builder().nondata());
        let ata = sat.response(&response).unwrap();
        assert_eq!(ata.status, 0x51, "ATA STATUS");
        assert_eq!(ata.error, 0x04, "ATA ERROR");
    }

    // 入力: 実バッファにはATA descriptorがあるが、sb_len_wr=8（senseヘッダのみ）。
    // 期待値: descriptor本体を読まず、SAT変換結果はNone。
    #[test]
    fn only_written_sense_bytes_are_parsed() {
        let mut hdr = completion_header(0x02);
        hdr.sb_len_wr = 8;
        let sense = [
            0x72, 0x0b, 0, 0, 0, 0, 0, 14, 0x09, 0x0c, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0x40, 0x51,
        ];
        let response = check_completion(&hdr, &sense).unwrap();
        let Some(SenseData::CurrentDescriptor(sense)) = &response.sense_data else {
            panic!("expected descriptor sense");
        };
        assert!(sense.sense_data_descriptor.is_empty(), "DESCRIPTORS");
        let sat = crate::scsi::sat::AtaPt::new(crate::ata::Command::builder().nondata());
        assert!(sat.response(&response).is_none());
    }

    // 入力: CHECK CONDITIONとhostエラー、またはdriverエラー。
    // 期待値: transport失敗としてErr。SCSI応答として成功扱いしない。
    #[test]
    fn host_and_driver_failures_remain_errors() {
        let mut host_failure = completion_header(0x02);
        host_failure.host_status = 1;
        let mut driver_failure = completion_header(0x02);
        driver_failure.driver_status = 4;
        for hdr in [host_failure, driver_failure] {
            assert!(matches!(
                check_completion(&hdr, &[]),
                Err(error) if matches!(error.kind, SgIoErrorKind::TransportError { .. })
            ));
        }
    }

    // 入力: GOODだが残量が1、または-1（overrun）。
    // 期待値: 全量転送の契約を保ち、UnexpectedResidualを返す。
    #[test]
    fn good_requires_a_complete_transfer() {
        for residual in [1, -1] {
            let mut hdr = completion_header(0);
            hdr.resid = residual;
            assert!(matches!(
                check_completion(&hdr, &[]),
                Err(error) if matches!(error.kind, SgIoErrorKind::UnexpectedResidual { residual: value, .. } if value == residual)
            ));
        }
    }

    // 入力: CHECK CONDITIONだが残量が-1、または要求長512を超える513。
    // 期待値: 不正な転送情報をUnexpectedResidualとして返す。
    #[test]
    fn check_condition_rejects_invalid_residuals() {
        for residual in [-1, 513] {
            let mut hdr = completion_header(0x02);
            hdr.resid = residual;
            assert!(matches!(
                check_completion(&hdr, &[]),
                Err(error) if matches!(error.kind, SgIoErrorKind::UnexpectedResidual { residual: value, .. } if value == residual)
            ));
        }
    }
}
