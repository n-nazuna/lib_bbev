use lib_bbev::ata::Command;
use lib_bbev::blk_cmd::{XferDirection, XferLength, XferParameter};
use lib_bbev::scsi::sat::AtaPt;

#[test]
fn ata_sat_preserves_transfer_direction_and_units() {
    for length in [
        XferLength::Pages(2),
        XferLength::Sectors(2),
        XferLength::Bytes(777),
    ] {
        for (command, expected_direction) in [
            (
                Command::builder().pio_data_in(length),
                XferDirection::TargetToInitiator(length),
            ),
            (
                Command::builder().pio_data_out(length),
                XferDirection::InitiatorToTarget(length),
            ),
            (
                Command::builder().dma_data_in(length),
                XferDirection::TargetToInitiator(length),
            ),
            (
                Command::builder().dma_data_out(length),
                XferDirection::InitiatorToTarget(length),
            ),
            (
                Command::builder().ncq_data_in(length),
                XferDirection::TargetToInitiator(length),
            ),
            (
                Command::builder().ncq_data_out(length),
                XferDirection::InitiatorToTarget(length),
            ),
        ] {
            let expected_transfer = XferParameter::XferDirection(expected_direction);
            let pass_through = AtaPt::new(command);
            for scsi in [pass_through.cdb16(), pass_through.cdb32()] {
                assert_eq!(scsi.xfer, expected_transfer);
            }
        }
    }
}

#[test]
fn nondata_protocols_use_no_transfer_and_an_empty_buffer() {
    for command in [
        Command::builder().nondata(),
        Command::builder().ncq_nondata(),
    ] {
        let pass_through = AtaPt::new(command);
        for scsi in [pass_through.cdb16(), pass_through.cdb32()] {
            assert_eq!(scsi.xfer, XferParameter::NoDataTransfer);
        }
    }
}
