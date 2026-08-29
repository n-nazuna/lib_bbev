use crate::scsi::{Cdb, Scsi};
use std::io;

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
    ScsiError { status: u8, host_status: u16, driver_status: u16 },
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

impl Cdb {
    fn as_slice(&self) -> &[u8] {
        match self {
            Cdb::cdb6(b) => &b[..],
            Cdb::cdb10(b) => &b[..],
            Cdb::cdb12(b) => &b[..],
            Cdb::cdb16(b) => &b[..],
        }
    }
}

impl Device {
    pub fn new(fd: i32, sector_size_bytes: u32, max_sectors_kbytes: u32) -> Self {
        Self { fd, sector_size_bytes, max_sectors_kbytes }
    }

    pub fn execute(&self, cmd: &impl Scsi, buf: &mut [u8]) -> Result<(), SgIoError> {
        let cdb = cmd.cdb();
        let cdb_slice = cdb.as_slice();
        let xfer = cmd.xfer_length();

        let dxfer_direction = match xfer.direction {
            XferDirection::TargetToInitiator => SG_DXFER_FROM_DEV,
            XferDirection::InitiatorToTarget => SG_DXFER_TO_DEV,
            XferDirection::NoDataTransfer => SG_DXFER_NONE,
        };

        let mut sense_buffer = [0u8; 32];
        let mut hdr = SgIoHdr {
            interface_id: b'S' as i32,
            dxfer_direction,
            cmd_len: cdb_slice.len() as u8,
            mx_sb_len: sense_buffer.len() as u8,
            iovec_count: 0,
            dxfer_len: buf.len() as u32,
            dxferp: buf.as_mut_ptr(),
            cmdp: cdb_slice.as_ptr(),
            sbp: sense_buffer.as_mut_ptr(),
            timeout: 20,
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
