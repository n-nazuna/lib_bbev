//! SAT.typ §7〜9 に基づく応答変換テスト。デバイスへの送信は行わない。

use lib_bbev::ata::Command as AtaCommand;
use lib_bbev::scsi::sat::AtaPt;
use lib_bbev::scsi::{DescriptorSense, FixedSense, Response, SenseData, Status};

fn descriptor_response(data: &[u8]) -> Response {
    Response {
        status: Status::CheckCondition,
        sense_data: Some(SenseData::CurrentDescriptor(DescriptorSense {
            sense_key: 0x0b,
            additional_sense_code: 0,
            additional_sense_qualifier: 0,
            additional_sense_length: data.len() as u8,
            sense_data_descriptor: data.to_vec(),
        })),
    }
}

fn fixed_response(flags: u8) -> Response {
    Response {
        status: Status::CheckCondition,
        sense_data: Some(SenseData::CurrentFixed(FixedSense {
            valid: false,
            filemark: false,
            end_of_medium: false,
            incorrect_length: false,
            sense_key: 0x0b,
            information: [0x04, 0x51, 0x4b, 0x34],
            additional_sense_length: 10,
            command_specific_information: [flags, 0x66, 0x55, 0x44],
            additional_sense_code: 0,
            additional_sense_qualifier: 0,
            field_replaceable_unit_code: 0,
            sense_key_specific: [0; 3],
            additional_sense: vec![],
        })),
    }
}

// SAT.typ §8
// 入力: 別descriptorの後にATA descriptor、EXTEND=1、COUNT=[12 34]、
//       LBA=[33 66 22 55 11 44]、ERROR=04、STATUS=51、DEVICE=40。
// 期待値: count=0x1234、lba=0x112233445566、error=0x04、status=0x51、device=0x40。
#[test]
fn descriptor_48bit_registers() {
    let scsi = descriptor_response(&[
        0x80, 0x02, 0, 0, // Skip an unrelated descriptor, including its payload.
        0x09, 0x0c, 0x01, 0x04, 0x12, 0x34, 0x33, 0x66, 0x22, 0x55, 0x11, 0x44, 0x40, 0x51,
    ]);
    let ata = AtaPt::new(AtaCommand::builder().nondata())
        .response(&scsi)
        .unwrap();
    assert_eq!(ata.error, 0x04, "ERROR");
    assert_eq!(ata.status, 0x51, "STATUS");
    let register = ata.register.unwrap();
    assert_eq!(register.count, 0x1234, "COUNT");
    assert_eq!(register.lba, 0x1122_3344_5566, "LBA");
    assert_eq!(register.device, 0x40, "DEVICE");
}

// SAT.typ §8
// 入力: EXTEND=0、COUNT=34、LBA=[0b 66 00 55 00 44]、DEVICE=4a。
// 期待値: count=0x34、lba=0x0b445566（上位4bitはdescriptor byte 6）、device=0x4a。
#[test]
fn descriptor_28bit_registers() {
    let scsi = descriptor_response(&[
        0x09, 0x0c, 0, 0x04, 0, 0x34, 0x0b, 0x66, 0, 0x55, 0, 0x44, 0x4a, 0x51,
    ]);
    let ata = AtaPt::new(AtaCommand::builder().nondata())
        .response(&scsi)
        .unwrap();
    let register = ata.register.unwrap();
    assert_eq!(register.count, 0x34, "COUNT");
    assert_eq!(register.lba, 0x0b44_5566, "LBA");
    assert_eq!(register.device, 0x4a, "DEVICE");
}

// SAT.typ §6、9
// 入力: VALID=0、INFORMATION=[04 51 4b 34]、LBA下位=[66 55 44]。
// 期待値: error=0x04、status=0x51、count=0x34、device=0x4b。
//         EXTEND=0ならlba=0x0b445566、EXTEND=1かつ上位値が0ならlba=0x445566。
#[test]
fn fixed_registers() {
    let pt = AtaPt::new(AtaCommand::builder().nondata());
    for (flags, expected_lba) in [(0x00, 0x0b44_5566), (0x80, 0x0044_5566)] {
        let ata = pt.response(&fixed_response(flags)).unwrap();
        assert_eq!(ata.error, 0x04, "ERROR");
        assert_eq!(ata.status, 0x51, "STATUS");
        let register = ata.register.unwrap();
        assert_eq!(register.count, 0x34, "COUNT");
        assert_eq!(register.lba, expected_lba, "LBA, flags={flags:#04x}");
        assert_eq!(register.device, 0x4b, "DEVICE");
    }
}

// SAT.typ §9
// 入力: EXTEND=1、COUNT UPPER NONZERO / LBA UPPER NONZEROの一方または両方が1。
// 期待値: error=0x04、status=0x51を保持、復元できないregisterはNone。
#[test]
fn fixed_missing_upper_values() {
    let pt = AtaPt::new(AtaCommand::builder().nondata());
    for flags in [0xc0, 0xa0, 0xe0] {
        let ata = pt.response(&fixed_response(flags)).unwrap();
        assert_eq!(ata.error, 0x04, "ERROR");
        assert_eq!(ata.status, 0x51, "STATUS");
        assert!(ata.register.is_none(), "flags={flags:#04x}");
    }
}

// 入力: descriptorなし、ヘッダ不足、本体不足、長さが0x0c以外、宣言長の外にATA descriptor。
// 期待値: いずれもNone。宣言長の外を読まず、不完全なレジスタ値を返さない。
#[test]
fn absent_or_short_descriptor() {
    let pt = AtaPt::new(AtaCommand::builder().nondata());
    for data in [vec![], vec![0x09], vec![0x09, 0x0c, 1], vec![0x09, 0]] {
        assert!(pt.response(&descriptor_response(&data)).is_none());
    }
    let mut scsi = descriptor_response(&[0x09, 0x0c, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x51]);
    let SenseData::CurrentDescriptor(sense) = scsi.sense_data.as_mut().unwrap() else {
        unreachable!();
    };
    sense.additional_sense_length = 0;
    assert!(pt.response(&scsi).is_none());
}

// 入力: GOODでsenseなし、両形式のdeferred sense、またはPIO Data-Inで01/00/1dの完了通知。
// 期待値: 現在のコマンドのATA値が得られないためNone。ゼロ値をレジスタとして返さない。
#[test]
fn no_current_ata_fields() {
    let pt = AtaPt::new(AtaCommand::builder().nondata());
    assert!(
        pt.response(&Response {
            status: Status::Good,
            sense_data: None
        })
        .is_none()
    );
    for mut scsi in [
        fixed_response(0),
        descriptor_response(&[0x09, 0x0c, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x51]),
    ] {
        scsi.sense_data = Some(match scsi.sense_data.take().unwrap() {
            SenseData::CurrentFixed(sense) => SenseData::DeferredFixed(sense),
            SenseData::CurrentDescriptor(sense) => SenseData::DeferredDescriptor(sense),
            _ => unreachable!(),
        });
        assert!(pt.response(&scsi).is_none());
    }

    let mut scsi = descriptor_response(&[0x09, 0x0c, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let SenseData::CurrentDescriptor(sense) = scsi.sense_data.as_mut().unwrap() else {
        unreachable!();
    };
    sense.sense_key = 0x01;
    sense.additional_sense_qualifier = 0x1d;
    let pt =
        AtaPt::new(AtaCommand::builder().pio_data_in(lib_bbev::blk_cmd::XferLength::Sectors(1)));
    assert!(pt.response(&scsi).is_none());
}
