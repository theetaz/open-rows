// SPDX-License-Identifier: Apache-2.0
//! Facet limits are enforced before accepting untrusted decoded collections.
use openrows_host_protocol::{
    Control, FACET_LIMIT, FacetReply, Frame, ProtocolError, WireRowEditInfo,
};
#[test]
fn oversized_fragment_is_refused_before_collection_allocation() -> Result<(), ProtocolError> {
    let frame = Frame::control(
        1,
        1,
        &Control::FacetChunk {
            sequence: 1,
            bytes: vec![0; 65537],
        },
    )?;
    assert_eq!(frame.decode_control(), Err(ProtocolError::Invalid));
    Ok(())
}
#[test]
fn native_nested_lists_are_refused_before_sending_a_facet() {
    let value = FacetReply::EditInfo(WireRowEditInfo {
        key: vec![String::new(); 4097],
        protected: vec![],
        row_id: None,
    });
    assert_eq!(value.encode(), Err(ProtocolError::Invalid));
}
#[test]
fn aggregate_facet_ceiling_is_checked_before_encoded_allocation() {
    let value = FacetReply::Fingerprint(Some("x".repeat(FACET_LIMIT)));
    assert_eq!(value.encode(), Err(ProtocolError::Limit));
}

#[test]
fn nested_collection_allocation_is_bounded_across_the_whole_facet()
-> Result<(), Box<dyn std::error::Error>> {
    let section = openrows_host_protocol::WireDetailSection {
        name: String::new(),
        columns: vec![],
        rows: vec![vec![]; 4096],
    };
    let value = FacetReply::ObjectDetail(openrows_host_protocol::WireObjectDetail {
        attributes: vec![],
        sections: vec![section; 200],
        definition: None,
    });
    let bytes = postcard::to_allocvec(&value)?;
    assert!(bytes.len() < 1024 * 1024);
    assert!(
        FacetReply::decode(&bytes).is_err(),
        "nested collection allocations exceed the aggregate budget"
    );
    Ok(())
}
