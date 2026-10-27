//! A dropped file on its way to a host: the shelf row shows the bytes that have gone.

use temor_core::shelf::{UploadId, UploadIn, upload_step};
use temor_core::vocab::ids::Bytes;

use super::support::Rig;
use temor_client::ssh::work::SshEvent;

#[test]
fn the_shelf_row_of_an_upload_moves_as_parts_arrive() {
    let Some(mut rig) = Rig::new("upload-progress") else {
        return;
    };
    let id = rig.hub.shelf.uploads.next_id();
    let list = std::mem::take(&mut rig.hub.shelf.uploads);
    rig.hub.shelf.uploads = upload_step(
        list,
        UploadIn::Started {
            name: "big.bin".to_owned(),
            host: "prod".to_owned(),
            folder: "/srv".to_owned(),
            size: Bytes(10_000_000),
        },
    );
    assert_eq!(id, UploadId(1));
    let mut seen = Vec::new();
    for sent in [4_000_000, 8_000_000] {
        let said = rig.hub.ssh_event(SshEvent::UploadProgress {
            id,
            sent: Bytes(sent),
        });
        assert!(said.is_empty(), "progress tells the person nothing");
        seen.push(rig.hub.shelf.uploads.items()[0].sent.0);
    }
    assert_eq!(seen, [4_000_000, 8_000_000]);
}
