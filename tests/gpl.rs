//! GPL.md §1〜3に基づくコマンド生成テスト。デバイスへの送信は行わない。

use lib_bbev::ata::gpl::{Gpl, GplBuilder};
use lib_bbev::ata::{Ata, AtaProtocol};
use lib_bbev::blk_cmd::{XferDirection, XferLength};

fn commands(builder: impl Fn() -> GplBuilder) -> [Ata; 4] {
    [
        builder().read_log_dma_ext(),
        builder().write_log_dma_ext(),
        builder().receive_fpdma_queued(),
        builder().send_fpdma_queued(),
    ]
}

// GPL.md §1〜2
// 入力: Read、address=0x12、page_number=0x3456、page_count=0x789a、feature=0xbcde。
// 期待値: COMMAND=0x47、FEATURE=0xbcde、COUNT=0x789a、LBA=0x003400005612。
//         Protocol=DMA、方向=デバイス→ホスト、長さ=0x789aページ（各512バイト）。
#[test]
fn read_log_dma_ext_registers_and_transfer() {
    let command = Gpl::builder()
        .address(0x12)
        .page_number(0x3456)
        .page_count(0x789a)
        .feature(0xbcde)
        .read_log_dma_ext();

    assert_eq!(command.command as u8, 0x47, "COMMAND");
    assert_eq!(command.feature, 0xbcde, "FEATURE");
    assert_eq!(command.count, 0x789a, "COUNT / LOG PAGE COUNT");
    assert_eq!(command.lba, 0x0034_0000_5612, "LBA");
    assert_eq!(command.aux, 0, "AUXILIARY");
    assert_eq!(command.device & 0x0f, 0, "DEVICE [3:0] reserved");
    let AtaProtocol::Dma { direction } = command.protocol else {
        panic!("expected DMA protocol");
    };
    assert_eq!(
        direction,
        XferDirection::TargetToInitiator(XferLength::Pages(0x789a)),
        "transfer direction and 512-byte page count"
    );
}

// GPL.md §1〜2
// 入力: Write、address=0xe0、page_number=0x1234、page_count=0x5678、feature=0xabcd。
// 期待値: COMMAND=0x57、FEATURE=0（予約）、COUNT=0x5678、LBA=0x0012000034e0。
//         Protocol=DMA、方向=ホスト→デバイス、長さ=0x5678ページ（各512バイト）。
#[test]
fn write_log_dma_ext_registers_and_transfer() {
    let command = Gpl::builder()
        .address(0xe0)
        .page_number(0x1234)
        .page_count(0x5678)
        .feature(0xabcd)
        .write_log_dma_ext();

    assert_eq!(command.command as u8, 0x57, "COMMAND");
    assert_eq!(command.feature, 0, "FEATURE reserved");
    assert_eq!(command.count, 0x5678, "COUNT / LOG PAGE COUNT");
    assert_eq!(command.lba, 0x0012_0000_34e0, "LBA");
    assert_eq!(command.aux, 0, "AUXILIARY");
    assert_eq!(command.device & 0x0f, 0, "DEVICE [3:0] reserved");
    let AtaProtocol::Dma { direction } = command.protocol else {
        panic!("expected DMA protocol");
    };
    assert_eq!(
        direction,
        XferDirection::InitiatorToTarget(XferLength::Pages(0x5678)),
        "transfer direction and 512-byte page count"
    );
}

// GPL.md §1・3
// 入力: NCQ Read、address=0x12、page_number=0x3456、page_count=0x789a、feature=0xbcde。
// 期待値: COMMAND=0x65、FEATURE=0x789a、LBA=0x003400005612、AUXILIARY=0x0000bcde。
//         COUNT: PRIO=0、SUBCOMMAND=1、NCQ TAG=0、Reserved=0。DEVICE=0x40。
//         Protocol=NCQ、方向=デバイス→ホスト、長さ=0x789aページ（各512バイト）。
#[test]
fn receive_fpdma_queued_encapsulates_read_log_dma_ext() {
    let command = Gpl::builder()
        .address(0x12)
        .page_number(0x3456)
        .page_count(0x789a)
        .feature(0xbcde)
        .receive_fpdma_queued();

    assert_eq!(command.command as u8, 0x65, "COMMAND");
    assert_eq!(command.feature, 0x789a, "FEATURE / transfer block count");
    assert_eq!(command.count >> 14, 0, "COUNT [15:14] / PRIO");
    assert_eq!((command.count >> 8) & 0x1f, 1, "COUNT [12:8] / SUBCOMMAND");
    assert_eq!((command.count >> 3) & 0x1f, 0, "COUNT [7:3] / NCQ TAG");
    assert_eq!(command.count & 0x2007, 0, "COUNT [13, 2:0] reserved");
    assert_eq!(command.lba, 0x0034_0000_5612, "LBA");
    assert_eq!(command.aux & 0xffff, 0xbcde, "AUXILIARY [15:0] / FEATURE");
    assert_eq!(command.aux >> 16, 0, "AUXILIARY [31:16] reserved");
    assert_eq!(command.device, 0x40, "DEVICE");
    let AtaProtocol::Ncq { direction } = command.protocol else {
        panic!("expected NCQ protocol");
    };
    assert_eq!(
        direction,
        XferDirection::TargetToInitiator(XferLength::Pages(0x789a)),
        "transfer direction and 512-byte page count"
    );
}

// GPL.md §1・3
// 入力: NCQ Write、address=0xe0、page_number=0x1234、page_count=0x5678、feature=0xabcd。
// 期待値: COMMAND=0x64、FEATURE=0x5678、LBA=0x0012000034e0、AUXILIARY=0（予約）。
//         COUNT: PRIO=0、SUBCOMMAND=2、NCQ TAG=0、Reserved=0。DEVICE=0x40。
//         Protocol=NCQ、方向=ホスト→デバイス、長さ=0x5678ページ（各512バイト）。
#[test]
fn send_fpdma_queued_encapsulates_write_log_dma_ext() {
    let command = Gpl::builder()
        .address(0xe0)
        .page_number(0x1234)
        .page_count(0x5678)
        .feature(0xabcd)
        .send_fpdma_queued();

    assert_eq!(command.command as u8, 0x64, "COMMAND");
    assert_eq!(command.feature, 0x5678, "FEATURE / transfer block count");
    assert_eq!(command.count >> 14, 0, "COUNT [15:14] / PRIO");
    assert_eq!((command.count >> 8) & 0x1f, 2, "COUNT [12:8] / SUBCOMMAND");
    assert_eq!((command.count >> 3) & 0x1f, 0, "COUNT [7:3] / NCQ TAG");
    assert_eq!(command.count & 0x2007, 0, "COUNT [13, 2:0] reserved");
    assert_eq!(command.lba, 0x0012_0000_34e0, "LBA");
    assert_eq!(command.aux, 0, "AUXILIARY reserved");
    assert_eq!(command.device, 0x40, "DEVICE");
    let AtaProtocol::Ncq { direction } = command.protocol else {
        panic!("expected NCQ protocol");
    };
    assert_eq!(
        direction,
        XferDirection::InitiatorToTarget(XferLength::Pages(0x5678)),
        "transfer direction and 512-byte page count"
    );
}

// GPL.md §1
// 入力: 全4コマンド、address=0xff、page_number=0x0000 / 0x00ff / 0x0100 / 0xffff。
// 期待値: LBAは下記expected_lba。ページ番号の上下バイトを分離し、予約領域は0。
#[test]
fn page_number_byte_boundaries_and_reserved_lba_bits() {
    struct TestCase {
        page_number: u16,
        expected_lba: u64,
    }
    let cases = [
        TestCase {
            page_number: 0x0000,
            expected_lba: 0x0000_0000_00ff,
        },
        TestCase {
            page_number: 0x00ff,
            expected_lba: 0x0000_0000_ffff,
        },
        TestCase {
            page_number: 0x0100,
            expected_lba: 0x0001_0000_00ff,
        },
        TestCase {
            page_number: 0xffff,
            expected_lba: 0x00ff_0000_ffff,
        },
    ];

    for case in cases {
        for command in commands(|| Gpl::builder().address(0xff).page_number(case.page_number)) {
            assert_eq!(
                command.lba, case.expected_lba,
                "LBA, PAGE NUMBER={:#06x}",
                case.page_number
            );
            assert_eq!(
                command.lba & 0xff00_ffff_0000,
                0,
                "LBA [47:40, 31:16] reserved"
            );
        }
    }
}

// GPL.md §2〜3
// 入力: 全4コマンド、page_count=1 / 0x0100 / 0xffff。
// 期待値: DMAはCOUNT、NCQはFEATUREが入力値。転送長も同じ512バイトページ数。
//         NCQのCOUNTはページ数に依存せず、Read=0x0100 / Write=0x0200。
#[test]
fn page_count_uses_full_16_bits() {
    for page_count in [1, 0x0100, 0xffff] {
        for command in commands(|| Gpl::builder().page_count(page_count)) {
            let direction = match command.protocol {
                AtaProtocol::Dma { direction } => {
                    assert_eq!(command.count, page_count, "COUNT / LOG PAGE COUNT");
                    direction
                }
                AtaProtocol::Ncq { direction } => {
                    assert_eq!(
                        command.feature, page_count,
                        "FEATURE / transfer block count"
                    );
                    let expected_count = match direction {
                        XferDirection::TargetToInitiator(_) => 0x0100,
                        XferDirection::InitiatorToTarget(_) => 0x0200,
                    };
                    assert_eq!(command.count, expected_count, "COUNT / SUBCOMMAND");
                    direction
                }
                _ => panic!("expected DMA or NCQ protocol"),
            };
            let length = match direction {
                XferDirection::TargetToInitiator(length)
                | XferDirection::InitiatorToTarget(length) => length,
            };
            assert_eq!(
                length,
                XferLength::Pages(u32::from(page_count)),
                "512-byte page count"
            );
        }
    }
}

// GPL.md §1〜2
// 入力: Gpl::builder() / GplBuilder::new() / GplBuilder::default() → DMA Read。
// 期待値: Directory（LOG ADDRESS=0）、PAGE NUMBER=0、COUNT=1、FEATURE=0、DMA In / Pages(1)。
#[test]
fn default_builder_reads_one_directory_page() {
    for builder in [Gpl::builder(), GplBuilder::new(), GplBuilder::default()] {
        let command = builder.read_log_dma_ext();
        assert_eq!(command.command as u8, 0x47, "COMMAND");
        assert_eq!(command.lba, 0, "LOG ADDRESS / PAGE NUMBER");
        assert_eq!(command.count, 1, "COUNT / LOG PAGE COUNT");
        assert_eq!(command.feature, 0, "FEATURE reserved");
        let AtaProtocol::Dma { direction } = command.protocol else {
            panic!("expected DMA protocol");
        };
        assert_eq!(
            direction,
            XferDirection::TargetToInitiator(XferLength::Pages(1))
        );
    }
}
