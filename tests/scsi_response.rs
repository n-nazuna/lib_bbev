use lib_bbev::scsi::SenseData;

// 入力: RESPONSE CODE=f0、byte 2=e5、追加長=11、末尾に未使用のパディング。
// 期待値: CurrentFixed、VALID/FILEMARK/EOM/ILI=true、sense_key=5。
//         INFORMATION=[04 51 40 34]、CSI=[80 66 55 44]、追加データは[aa]のみ。
#[test]
fn parse_fixed_sense() {
    let bytes = [
        0xf0, 0, 0xe5, 0x04, 0x51, 0x40, 0x34, 11, 0x80, 0x66, 0x55, 0x44, 0x20, 1, 2, 0x80, 3, 4,
        0xaa, 0,
    ];
    let Some(SenseData::CurrentFixed(sense)) = SenseData::from_bytes(&bytes) else {
        panic!("expected current fixed sense");
    };
    assert!(sense.valid, "VALID");
    assert!(sense.filemark, "FILEMARK");
    assert!(sense.end_of_medium, "EOM");
    assert!(sense.incorrect_length, "ILI");
    assert_eq!(sense.sense_key, 5, "SENSE KEY");
    assert_eq!(sense.information, [0x04, 0x51, 0x40, 0x34], "INFORMATION");
    assert_eq!(sense.additional_sense_length, 11, "ADDITIONAL SENSE LENGTH");
    assert_eq!(
        sense.command_specific_information,
        [0x80, 0x66, 0x55, 0x44],
        "CSI"
    );
    assert_eq!(sense.additional_sense_code, 0x20, "ASC");
    assert_eq!(sense.additional_sense_qualifier, 1, "ASCQ");
    assert_eq!(sense.field_replaceable_unit_code, 2, "FRU CODE");
    assert_eq!(sense.sense_key_specific, [0x80, 3, 4], "SENSE KEY SPECIFIC");
    assert_eq!(sense.additional_sense, [0xaa], "ADDITIONAL SENSE BYTES");
}

// 入力: descriptor形式、sense_key=0b、ASC/ASCQ=00/1d、追加長=14、後続にパディング。
// 期待値: descriptor本体のみ取り出し、SAT変換でSTATUS=51、ERROR=04、LBA=112233445566。
#[test]
fn parse_descriptor_sense_and_convert_to_ata() {
    let bytes = [
        0x72, 0x0b, 0, 0x1d, 0, 0, 0, 14, 0x09, 0x0c, 1, 0x04, 0x12, 0x34, 0x33, 0x66, 0x22, 0x55,
        0x11, 0x44, 0x40, 0x51, 0, 0,
    ];
    let Some(SenseData::CurrentDescriptor(sense)) = SenseData::from_bytes(&bytes) else {
        panic!("expected current descriptor sense");
    };
    assert_eq!(sense.sense_key, 0x0b, "SENSE KEY");
    assert_eq!(sense.additional_sense_code, 0, "ASC");
    assert_eq!(sense.additional_sense_qualifier, 0x1d, "ASCQ");
    assert_eq!(sense.additional_sense_length, 14, "ADDITIONAL SENSE LENGTH");
    assert_eq!(sense.sense_data_descriptor, bytes[8..22], "DESCRIPTORS");

    let response = lib_bbev::scsi::Response {
        status: lib_bbev::scsi::Status::CheckCondition,
        sense_data: Some(SenseData::CurrentDescriptor(sense)),
    };
    let ata = lib_bbev::scsi::sat::AtaPt::new(lib_bbev::ata::Command::builder().nondata())
        .response(&response)
        .unwrap();
    assert_eq!(ata.status, 0x51, "STATUS");
    assert_eq!(ata.error, 0x04, "ERROR");
    let register = ata.register.unwrap();
    assert_eq!(register.count, 0x1234, "COUNT");
    assert_eq!(register.lba, 0x1122_3344_5566, "LBA");
    assert_eq!(register.device, 0x40, "DEVICE");
}

// 入力: 71/73のdeferred sense、および不明なコード／短いヘッダ。
// 期待値: DeferredFixed/DeferredDescriptorに分類し、不明・不足ならNone。
#[test]
fn deferred_and_incomplete_sense() {
    let mut fixed = [0; 18];
    fixed[0] = 0x71;
    fixed[7] = 10;
    assert!(matches!(
        SenseData::from_bytes(&fixed),
        Some(SenseData::DeferredFixed(_))
    ));
    assert!(matches!(
        SenseData::from_bytes(&[0x73, 0, 0, 0, 0, 0, 0, 0]),
        Some(SenseData::DeferredDescriptor(_))
    ));
    assert!(SenseData::from_bytes(&[]).is_none());
    assert!(SenseData::from_bytes(&[0x72; 7]).is_none());
    assert!(SenseData::from_bytes(&[0; 18]).is_none());
    assert!(SenseData::from_bytes(&fixed[..17]).is_none());
}
