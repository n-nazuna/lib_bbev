//! SAT.md §3〜6.1 に基づくCDB生成テスト。デバイスへの送信は行わない。

use lib_bbev::ata::{Ata, AtaCmd};
use lib_bbev::blk_cmd::{XferDirection, XferLength, XferParameter};
use lib_bbev::scsi::sat::AtaPt;
use lib_bbev::scsi::{Cdb, Scsi};

fn conversions(command: Ata) -> [Scsi; 3] {
    let pt = AtaPt::new(command);
    [pt.cdb12(), pt.cdb16(), pt.cdb32()]
}

// flags: CDB12/16のByte 2、CDB32のByte 11に格納する転送フラグ1バイト。
// bit 7:6=OFF_LINE、5=CK_COND、4=T_TYPE、3=T_DIR、2=BYTE_BLOCK、1:0=T_LENGTH。
fn protocol_and_flags(scsi: &Scsi) -> (u8, u8) {
    let (protocol, flags) = match &scsi.cdb {
        Cdb::Cdb12(cdb) => {
            assert_eq!(cdb[1] & 0xe1, 0, "CDB12 reserved/obsolete bits");
            (cdb[1], cdb[2])
        }
        Cdb::Cdb16(cdb) => {
            assert_eq!(cdb[1] & 0xe0, 0, "CDB16 obsolete bits");
            assert_eq!(cdb[1] & 1, 1, "CDB16 EXTEND");
            (cdb[1], cdb[2])
        }
        Cdb::Cdb32(cdb) => {
            assert_eq!(cdb[10] & 0xe0, 0, "CDB32 reserved bits");
            assert_eq!(cdb[10] & 1, 1, "CDB32 EXTEND");
            (cdb[10], cdb[11])
        }
        cdb => panic!("unexpected CDB: {cdb:?}"),
    };
    (protocol >> 1, flags)
}

// SAT.md §3
// 入力: ReadSectors / PIO In / Sectors(0x34)、feature=0x12、count=0x34、
//       lba=0x56789a、device=0x4b、control=0x80 → cdb12()
// 期待値: [a1 08 1e 12 34 9a 78 56 4b 20 00 80]（全て16進数）。
//         LBAは下位バイトから格納、DEVICEの下位4bitを保持、予約Byte 10は0。
#[test]
fn cdb12_register_layout() {
    let command = Ata::builder()
        .command(AtaCmd::ReadSectors)
        .feature(0x12)
        .count(0x34)
        .lba(0x56_789a)
        .device(0x4b)
        .control(0x80)
        .pio_data_in(XferLength::Sectors(0x34));
    let scsi = AtaPt::new(command).cdb12();

    let Cdb::Cdb12(cdb) = scsi.cdb else {
        panic!("expected CDB12");
    };
    assert_eq!(
        cdb,
        [
            0xa1, 0x08, 0x1e, 0x12, 0x34, 0x9a, 0x78, 0x56, 0x4b, 0x20, 0, 0x80
        ]
    );
}

// SAT.md §4
// 入力: ReadSectorsExt / PIO In / Sectors(0x99aa)、feature=0x7788、count=0x99aa、
//       lba=0x112233445566、device=0x40、control=0x80 → cdb16()
// 期待値: [85 09 1e 77 88 99 aa 33 66 22 55 11 44 40 24 80]（全て16進数）。
//         EXTEND=1、LBAの格納順は [33 66 22 55 11 44]。
#[test]
fn cdb16_register_layout() {
    let command = Ata::builder()
        .command(AtaCmd::ReadSectorsExt)
        .feature(0x7788)
        .count(0x99aa)
        .lba(0x1122_3344_5566)
        .device(0x40)
        .control(0x80)
        .pio_data_in(XferLength::Sectors(0x99aa));
    let scsi = AtaPt::new(command).cdb16();

    let Cdb::Cdb16(cdb) = scsi.cdb else {
        panic!("expected CDB16");
    };
    assert_eq!(
        cdb,
        [
            0x85, 0x09, 0x1e, 0x77, 0x88, 0x99, 0xaa, 0x33, 0x66, 0x22, 0x55, 0x11, 0x44, 0x40,
            0x24, 0x80,
        ]
    );
}

// SAT.md §5
// 入力: ReadDmaExt / DMA In / Sectors(0x99aa)、feature=0x7788、count=0x99aa、
//       lba=0x112233445566、device=0x40、control=0x80、icc=0x77、aux=0x8899aabb → cdb32()
// 期待値（全て16進数）:
//   [7f 80 00 00 00 00 00 18 1f f0 0d 1e 00 00 11 22
//    33 44 55 66 77 88 99 aa 40 25 00 77 88 99 aa bb]
// 追加長=0x18、service action=0x1ff0、EXTEND=1。レジスタは上位バイトから格納。
#[test]
fn cdb32_register_layout() {
    let command = Ata::builder()
        .command(AtaCmd::ReadDmaExt)
        .feature(0x7788)
        .count(0x99aa)
        .lba(0x1122_3344_5566)
        .device(0x40)
        .control(0x80)
        .icc(0x77)
        .aux(0x8899_aabb)
        .dma_data_in(XferLength::Sectors(0x99aa));
    let scsi = AtaPt::new(command).cdb32();

    let Cdb::Cdb32(cdb) = scsi.cdb else {
        panic!("expected CDB32");
    };
    assert_eq!(
        cdb,
        [
            0x7f, 0x80, 0, 0, 0, 0, 0, 0x18, 0x1f, 0xf0, 0x0d, 0x1e, 0, 0, 0x11, 0x22, 0x33, 0x44,
            0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0x40, 0x25, 0, 0x77, 0x88, 0x99, 0xaa, 0xbb,
        ]
    );
}

struct ExpectedFields {
    protocol: u8,
    t_type: u8,
    t_dir: u8,
    byte_block: u8,
    t_length: u8,
}

struct DataTestCase {
    input: Ata,
    expected_fields: ExpectedFields,
    expected_transfer: XferParameter,
}

fn assert_fields(scsi: &Scsi, expected: &ExpectedFields) {
    let (protocol, flags) = protocol_and_flags(scsi);
    assert_eq!(protocol, expected.protocol, "PROTOCOL: {:?}", scsi.cdb);
    assert_eq!((flags >> 4) & 1, expected.t_type, "T_TYPE: {:?}", scsi.cdb);
    assert_eq!((flags >> 3) & 1, expected.t_dir, "T_DIR: {:?}", scsi.cdb);
    assert_eq!(
        (flags >> 2) & 1,
        expected.byte_block,
        "BYTE_BLOCK: {:?}",
        scsi.cdb
    );
    assert_eq!(flags & 0b11, expected.t_length, "T_LENGTH: {:?}", scsi.cdb);
    assert_eq!(flags >> 6, 0, "OFF_LINE: {:?}", scsi.cdb);
    assert_eq!((flags >> 5) & 1, 0, "CK_COND: {:?}", scsi.cdb);
}

// SAT.md §3.1〜3.2
// 入力: count=feature=2、長さ=Sectors(2)、CDB12/16/32。
// 各ケースのinput → expected_fields（CDBの各フィールド）とexpected_transfer（転送情報）。
// 共通の期待値: BYTE_BLOCK=1、T_TYPE=1。T_DIRはIn=1 / Out=0。
// T_LENGTHはREAD/WRITE FPDMAが01（FEATURES参照）、それ以外は10（COUNT参照）。
#[test]
fn data_protocols_and_directions() {
    use XferDirection::{InitiatorToTarget, TargetToInitiator};
    let length = XferLength::Sectors(2);
    let builder = || Ata::builder().count(2).feature(2);
    let cases = [
        DataTestCase {
            input: builder().command(AtaCmd::ReadSectors).pio_data_in(length),
            expected_fields: ExpectedFields {
                protocol: 0x04,
                t_type: 1,
                t_dir: 1,
                byte_block: 1,
                t_length: 0b10,
            },
            expected_transfer: XferParameter::XferDirection(TargetToInitiator(length)),
        },
        DataTestCase {
            input: builder().command(AtaCmd::WriteSectors).pio_data_out(length),
            expected_fields: ExpectedFields {
                protocol: 0x05,
                t_type: 1,
                t_dir: 0,
                byte_block: 1,
                t_length: 0b10,
            },
            expected_transfer: XferParameter::XferDirection(InitiatorToTarget(length)),
        },
        DataTestCase {
            input: builder().command(AtaCmd::ReadDma).dma_data_in(length),
            expected_fields: ExpectedFields {
                protocol: 0x06,
                t_type: 1,
                t_dir: 1,
                byte_block: 1,
                t_length: 0b10,
            },
            expected_transfer: XferParameter::XferDirection(TargetToInitiator(length)),
        },
        DataTestCase {
            input: builder().command(AtaCmd::WriteDma).dma_data_out(length),
            expected_fields: ExpectedFields {
                protocol: 0x06,
                t_type: 1,
                t_dir: 0,
                byte_block: 1,
                t_length: 0b10,
            },
            expected_transfer: XferParameter::XferDirection(InitiatorToTarget(length)),
        },
        DataTestCase {
            input: builder()
                .command(AtaCmd::ReadFpdmaQueued)
                .ncq_data_in(length),
            expected_fields: ExpectedFields {
                protocol: 0x0c,
                t_type: 1,
                t_dir: 1,
                byte_block: 1,
                t_length: 0b01,
            },
            expected_transfer: XferParameter::XferDirection(TargetToInitiator(length)),
        },
        DataTestCase {
            input: builder()
                .command(AtaCmd::WriteFpdmaQueued)
                .ncq_data_out(length),
            expected_fields: ExpectedFields {
                protocol: 0x0c,
                t_type: 1,
                t_dir: 0,
                byte_block: 1,
                t_length: 0b01,
            },
            expected_transfer: XferParameter::XferDirection(InitiatorToTarget(length)),
        },
    ];
    for case in cases {
        for scsi in conversions(case.input) {
            assert_fields(&scsi, &case.expected_fields);
            assert_eq!(
                scsi.xfer, case.expected_transfer,
                "transfer: {:?}",
                scsi.cdb
            );
        }
    }
}

// SAT.md §3.2
// 入力: ReadSectors / PIO In または WriteSectors / PIO Out、count=2、CDB12/16/32。
// 長さ → 期待値 (BYTE_BLOCK, T_TYPE):
//   Bytes(2)   → (0, 0)
//   Pages(2)   → (1, 0)
//   Sectors(2) → (1, 1)
// InはPROTOCOL=4 / T_DIR=1、OutはPROTOCOL=5 / T_DIR=0、T_LENGTH=10（COUNT参照）。
#[test]
fn transfer_units_in_both_directions() {
    struct UnitTestCase {
        input_length: XferLength,
        expected_byte_block: u8,
        expected_t_type: u8,
    }
    let cases = [
        UnitTestCase {
            input_length: XferLength::Bytes(2),
            expected_byte_block: 0,
            expected_t_type: 0,
        },
        UnitTestCase {
            input_length: XferLength::Pages(2),
            expected_byte_block: 1,
            expected_t_type: 0,
        },
        UnitTestCase {
            input_length: XferLength::Sectors(2),
            expected_byte_block: 1,
            expected_t_type: 1,
        },
    ];
    for case in cases {
        let length = case.input_length;
        let directions = [
            DataTestCase {
                input: Ata::builder()
                    .command(AtaCmd::ReadSectors)
                    .count(2)
                    .pio_data_in(length),
                expected_fields: ExpectedFields {
                    protocol: 0x04,
                    t_type: case.expected_t_type,
                    t_dir: 1,
                    byte_block: case.expected_byte_block,
                    t_length: 0b10,
                },
                expected_transfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                    length,
                )),
            },
            DataTestCase {
                input: Ata::builder()
                    .command(AtaCmd::WriteSectors)
                    .count(2)
                    .pio_data_out(length),
                expected_fields: ExpectedFields {
                    protocol: 0x05,
                    t_type: case.expected_t_type,
                    t_dir: 0,
                    byte_block: case.expected_byte_block,
                    t_length: 0b10,
                },
                expected_transfer: XferParameter::XferDirection(XferDirection::InitiatorToTarget(
                    length,
                )),
            },
        ];
        for direction in directions {
            for scsi in conversions(direction.input) {
                assert_fields(&scsi, &direction.expected_fields);
                assert_eq!(
                    scsi.xfer, direction.expected_transfer,
                    "transfer: {:?}",
                    scsi.cdb
                );
            }
        }
    }
}

// SAT.md §3.2、§6.1
// 入力: NCQ、feature=0x1234、count=0x00a8（タグ）、CDB16/32。
// READ/WRITE FPDMAのSectors(0x1234) → T_TYPE=1。
// RECEIVE/SEND FPDMAのPages(0x1234) → T_TYPE=0。
// 共通の期待値: PROTOCOL=0x0c、BYTE_BLOCK=1、T_LENGTH=01（FEATURES参照）。
// FEATURES=[12 34]、COUNT=[00 a8]（16進数）、転送情報の長さは0x1234。
#[test]
fn fpdma_transfer_length_comes_from_features() {
    let sectors = XferLength::Sectors(0x1234);
    let pages = XferLength::Pages(0x1234);
    let builder = || Ata::builder().feature(0x1234).count(0x00a8);
    let cases = [
        DataTestCase {
            input: builder()
                .command(AtaCmd::ReadFpdmaQueued)
                .ncq_data_in(sectors),
            expected_fields: ExpectedFields {
                protocol: 0x0c,
                t_type: 1,
                t_dir: 1,
                byte_block: 1,
                t_length: 0b01,
            },
            expected_transfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                sectors,
            )),
        },
        DataTestCase {
            input: builder()
                .command(AtaCmd::WriteFpdmaQueued)
                .ncq_data_out(sectors),
            expected_fields: ExpectedFields {
                protocol: 0x0c,
                t_type: 1,
                t_dir: 0,
                byte_block: 1,
                t_length: 0b01,
            },
            expected_transfer: XferParameter::XferDirection(XferDirection::InitiatorToTarget(
                sectors,
            )),
        },
        DataTestCase {
            input: builder()
                .command(AtaCmd::ReceiveFpdmaQueued)
                .ncq_data_in(pages),
            expected_fields: ExpectedFields {
                protocol: 0x0c,
                t_type: 0,
                t_dir: 1,
                byte_block: 1,
                t_length: 0b01,
            },
            expected_transfer: XferParameter::XferDirection(XferDirection::TargetToInitiator(
                pages,
            )),
        },
        DataTestCase {
            input: builder()
                .command(AtaCmd::SendFpdmaQueued)
                .ncq_data_out(pages),
            expected_fields: ExpectedFields {
                protocol: 0x0c,
                t_type: 0,
                t_dir: 0,
                byte_block: 1,
                t_length: 0b01,
            },
            expected_transfer: XferParameter::XferDirection(XferDirection::InitiatorToTarget(
                pages,
            )),
        },
    ];
    for case in cases {
        let pt = AtaPt::new(case.input);
        for scsi in [pt.cdb16(), pt.cdb32()] {
            assert_fields(&scsi, &case.expected_fields);
            let (features, count) = match &scsi.cdb {
                Cdb::Cdb16(cdb) => (&cdb[3..5], &cdb[5..7]),
                Cdb::Cdb32(cdb) => (&cdb[20..22], &cdb[22..24]),
                _ => unreachable!(),
            };
            assert_eq!(features, &[0x12, 0x34], "FEATURES");
            assert_eq!(count, &[0, 0xa8], "COUNT");
            assert_eq!(
                scsi.xfer, case.expected_transfer,
                "transfer: {:?}",
                scsi.cdb
            );
        }
    }
}

// SAT.md §3.2
// 入力 → 期待値 (PROTOCOL, T_LENGTH, 転送情報)、CDB12/16/32:
//   FlushCache / NonData、count=0xff       → (0x03, 00, NoDataTransfer)
//   NcqNonData / NcqNonData、feature=0xff  → (0x0c, 00, NoDataTransfer)
// T_LENGTHは二進数。レジスタ値が非ゼロでも無転送とし、T_DIR/BYTE_BLOCK/T_TYPEは問わない。
#[test]
fn nondata_protocols_disable_transfer() {
    struct TestCase {
        input: Ata,
        expected_protocol: u8,
        expected_t_length: u8,
        expected_transfer: XferParameter,
    }
    let cases = [
        TestCase {
            input: Ata::builder()
                .command(AtaCmd::FlushCache)
                .count(0xff)
                .nondata(),
            expected_protocol: 0x03,
            expected_t_length: 0b00,
            expected_transfer: XferParameter::NoDataTransfer,
        },
        TestCase {
            input: Ata::builder()
                .command(AtaCmd::NcqNonData)
                .feature(0xff)
                .ncq_nondata(),
            expected_protocol: 0x0c,
            expected_t_length: 0b00,
            expected_transfer: XferParameter::NoDataTransfer,
        },
    ];
    for case in cases {
        for scsi in conversions(case.input) {
            let (protocol, flags) = protocol_and_flags(&scsi);
            assert_eq!(protocol, case.expected_protocol, "PROTOCOL: {:?}", scsi.cdb);
            assert_eq!(
                flags & 0b11,
                case.expected_t_length,
                "T_LENGTH: {:?}",
                scsi.cdb
            );
            assert_eq!(
                scsi.xfer, case.expected_transfer,
                "transfer: {:?}",
                scsi.cdb
            );
        }
    }
}
