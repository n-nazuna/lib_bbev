pub mod sat;
use crate::blk_cmd::XferParameter;

#[derive(Debug)]
pub enum Cdb {
    Cdb32([u8; 32]),
    Cdb16([u8; 16]),
    Cdb12([u8; 12]),
    Cdb10([u8; 10]),
    Cdb6([u8; 6]),
}
pub struct Command {
    pub cdb: Cdb,
    pub xfer: XferParameter,
}
impl Cdb {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Cdb::Cdb32(cdb) => cdb,
            Cdb::Cdb16(cdb) => cdb,
            Cdb::Cdb12(cdb) => cdb,
            Cdb::Cdb10(cdb) => cdb,
            Cdb::Cdb6(cdb) => cdb,
        }
    }
}
pub enum Status {
    Good,
    CheckCondition,
    Busy,
    ReservationConflict,
}
pub enum SenseData {
    CurrentFixed(FixedSense),
    CurrentDescriptor(DescriptorSense),
    DeferredFixed(FixedSense),
    DeferredDescriptor(DescriptorSense),
}
impl SenseData {
    /// Parses sense bytes, limiting the payload to ADDITIONAL SENSE LENGTH.
    /// Returns None for an unknown format or an incomplete header.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let additional_sense_length = *bytes.get(7)?;
        let length = bytes.len().min(8 + usize::from(additional_sense_length));
        let bytes = &bytes[..length];
        match bytes[0] & 0x7f {
            code @ (0x70 | 0x71) => {
                let header = bytes.get(..18)?;
                let sense = FixedSense {
                    valid: header[0] & 0x80 != 0,
                    filemark: header[2] & 0x80 != 0,
                    end_of_medium: header[2] & 0x40 != 0,
                    incorrect_length: header[2] & 0x20 != 0,
                    sense_key: header[2] & 0x0f,
                    information: header[3..7].try_into().ok()?,
                    additional_sense_length,
                    command_specific_information: header[8..12].try_into().ok()?,
                    additional_sense_code: header[12],
                    additional_sense_qualifier: header[13],
                    field_replaceable_unit_code: header[14],
                    sense_key_specific: header[15..18].try_into().ok()?,
                    additional_sense: bytes[18..].to_vec(),
                };
                Some(if code == 0x70 {
                    Self::CurrentFixed(sense)
                } else {
                    Self::DeferredFixed(sense)
                })
            }
            code @ (0x72 | 0x73) => {
                let sense = DescriptorSense {
                    sense_key: bytes[1] & 0x0f,
                    additional_sense_code: bytes[2],
                    additional_sense_qualifier: bytes[3],
                    additional_sense_length,
                    sense_data_descriptor: bytes[8..].to_vec(),
                };
                Some(if code == 0x72 {
                    Self::CurrentDescriptor(sense)
                } else {
                    Self::DeferredDescriptor(sense)
                })
            }
            _ => None,
        }
    }
}
pub struct FixedSense {
    pub valid: bool,
    pub filemark: bool,
    pub end_of_medium: bool,
    pub incorrect_length: bool,
    pub sense_key: u8,
    pub information: [u8; 4],
    pub additional_sense_length: u8,
    pub command_specific_information: [u8; 4],
    pub additional_sense_code: u8,
    pub additional_sense_qualifier: u8,
    pub field_replaceable_unit_code: u8,
    pub sense_key_specific: [u8; 3],
    pub additional_sense: Vec<u8>,
}

pub struct DescriptorSense {
    pub sense_key: u8,
    pub additional_sense_code: u8,
    pub additional_sense_qualifier: u8,
    pub additional_sense_length: u8,
    pub sense_data_descriptor: Vec<u8>,
}
pub struct Response {
    pub status: Status,
    pub sense_data: Option<SenseData>,
}
