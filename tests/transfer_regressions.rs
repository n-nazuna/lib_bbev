use lib_bbev::ata::{Ata, AtaCmd, AtaProtocol, Gpl};
use lib_bbev::scsi::{AtaPt, Cdb, Read16};
use lib_bbev::sg_io::{Device, SgIoErrorKind, XferDirection, XferLength, XferParameter};

#[test]
fn sat_direction_and_units_match_host_buffer_lengths() {
    // Independent SAT vectors: T_TYPE=bit 4, T_DIR=bit 3, BYTE_BLOCK=bit 2.
    let cases = [
        (XferLength::Pages(2), 2, 0x0e, 0x06, 1024, 1024),
        (XferLength::Sectors(2), 2, 0x1e, 0x16, 1024, 8192),
        (XferLength::Bytes(777), 777, 0x0a, 0x02, 777, 777),
    ];

    for (length, count, read_byte, write_byte, bytes_512, bytes_4k) in cases {
        for read in [true, false] {
            let direction = if read {
                XferDirection::TargetToInitiator(length)
            } else {
                XferDirection::InitiatorToTarget(length)
            };
            let command = Ata::new()
                .count(count)
                .command(if read { AtaCmd::ReadDmaExt } else { AtaCmd::WriteDmaExt })
                .protocol(AtaProtocol::Dma { direction });
            let pt = AtaPt::new(command);
            let scsi16 = pt.cdb16();
            let scsi32 = pt.cdb32();
            let expected = if read { read_byte } else { write_byte };
            let Cdb::Cdb16(cdb16) = &scsi16.cdb else { panic!("expected CDB16") };
            let Cdb::Cdb32(cdb32) = &scsi32.cdb else { panic!("expected CDB32") };
            assert_eq!(cdb16[2], expected);
            assert_eq!(cdb32[11], expected);
            assert_eq!(&cdb16[5..7], &count.to_be_bytes());
            assert_eq!(&cdb32[22..24], &count.to_be_bytes());

            for scsi in [&scsi16, &scsi32] {
                for (sector_size, expected_bytes) in [(512, bytes_512), (4096, bytes_4k)] {
                    let device = Device::new(-1, sector_size, 16);
                    assert_eq!(device.allocate(scsi).unwrap().len(), expected_bytes);
                }
            }
        }
    }
}

#[test]
fn gpl_read_and_write_encode_page_counts_and_ncq_subcommands() {
    // The SEND and RECEIVE subcommand numbers intentionally differ.
    for (ncq, read, expected_command, expected_count, expected_control) in [
        (false, true, 0x47, 2u16, 0x0e),
        (false, false, 0x57, 2u16, 0x06),
        (true, true, 0x65, 0x0100u16, 0x0d),
        (true, false, 0x64, 0x0200u16, 0x05),
    ] {
        let gpl = Gpl::new().feature(0x1234).page_number(0x5678).page_count(2).address(0x9a);
        let command = match (ncq, read) {
            (false, true) => gpl.read_log_dma_ext(),
            (false, false) => gpl.write_log_dma_ext(),
            (true, true) => gpl.receive_fpdma_queued(),
            (true, false) => gpl.send_fpdma_queued(),
        };
        let expected_feature = if ncq { 2u16 } else { 0x1234u16 };
        let fis = command.fis();
        assert_eq!(fis[2], expected_command);
        assert_eq!(fis[3], expected_feature as u8);
        assert_eq!(fis[11], (expected_feature >> 8) as u8);
        assert_eq!(&fis[12..14], &expected_count.to_le_bytes());
        assert_eq!(&fis[16..20], &(if ncq { 0x1234u32 } else { 0 }).to_le_bytes());

        let pt = AtaPt::new(command);
        for scsi in [pt.cdb16(), pt.cdb32()] {
            match &scsi.cdb {
                Cdb::Cdb16(cdb) => {
                    assert_eq!(cdb[2], expected_control);
                    assert_eq!(&cdb[3..5], &expected_feature.to_be_bytes());
                    assert_eq!(&cdb[5..7], &expected_count.to_be_bytes());
                    assert_eq!(cdb[14], expected_command);
                    assert_eq!([cdb[7], cdb[8], cdb[9], cdb[10], cdb[11], cdb[12]],
                               [0, 0x9a, 0x56, 0x78, 0, 0]);
                }
                Cdb::Cdb32(cdb) => {
                    assert_eq!(cdb[11], expected_control);
                    assert_eq!(&cdb[20..22], &expected_feature.to_be_bytes());
                    assert_eq!(&cdb[22..24], &expected_count.to_be_bytes());
                    assert_eq!(cdb[25], expected_command);
                    assert_eq!(&cdb[14..20], &[0, 0x56, 0, 0, 0x78, 0x9a]);
                    assert_eq!(&cdb[28..32], &(if ncq { 0x1234u32 } else { 0 }).to_be_bytes());
                }
                _ => panic!("unexpected CDB size"),
            }
            assert!(if read {
                matches!(scsi.xfer, XferParameter::XferDirection(
                    XferDirection::TargetToInitiator(XferLength::Pages(2))))
            } else {
                matches!(scsi.xfer, XferParameter::XferDirection(
                    XferDirection::InitiatorToTarget(XferLength::Pages(2))))
            });
            // GPL pages remain 512 bytes on a 4Kn device.
            assert_eq!(Device::new(-1, 4096, 16).allocate(&scsi).unwrap().len(), 1024);
        }
    }
}

#[test]
fn scsi_read16_preserves_64bit_lba() {
    let Cdb::Cdb16(cdb) = Read16::new().lba(u64::MAX).transfer_length(1).build().cdb
    else { panic!("expected CDB16") };
    assert_eq!(&cdb[2..10], &[0xff; 8]);
}

#[test]
fn four_kn_reads_enforce_byte_limits_before_ioctl() {
    let device = Device::new(-1, 4096, 8);
    let command = Read16::new().transfer_length(2).build();
    assert_eq!(device.allocate(&command).unwrap().len(), 8192);
    let error = device.execute(&command, &mut [0u8; 512]).unwrap_err();
    assert!(matches!(error.kind, SgIoErrorKind::TransferLengthMismatch {
        expected: 8192, actual: 512,
    }));
    let oversized = Read16::new().transfer_length(3).build();
    let error = device.allocate(&oversized).unwrap_err();
    assert!(matches!(error.kind, SgIoErrorKind::TransferTooLarge {
        requested: 12288, maximum: 8192,
    }));
}
