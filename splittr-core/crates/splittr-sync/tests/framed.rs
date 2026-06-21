//! The [`FramedTransport`] over a real async byte stream (a tokio duplex) — the
//! same length-prefixed code path an iroh QUIC bi-stream drives. Proves the
//! framing carries a full sync to convergence end-to-end.

mod common;

use common::{key, replica, world};
use splittr_sync::{sync, FramedTransport, Role, SyncStore};

#[tokio::test]
async fn replicas_converge_over_a_framed_byte_stream() {
    let (fa, fb) = (key(21), key(22));
    let mut a = replica();
    let mut b = replica();
    for o in world(&fa, "a") {
        a.append(&o).unwrap();
    }
    for o in world(&fb, "b") {
        b.append(&o).unwrap();
    }

    // A connected byte duplex: bytes written to one end are read at the other.
    let (sa, sb) = tokio::io::duplex(16 * 1024);
    let (ar, aw) = tokio::io::split(sa);
    let (br, bw) = tokio::io::split(sb);
    let mut ta = FramedTransport::new(ar, aw);
    let mut tb = FramedTransport::new(br, bw);

    let (ra, rb) = tokio::join!(
        sync(&mut a, &mut ta, Role::Initiator),
        sync(&mut b, &mut tb, Role::Responder),
    );
    ra.unwrap();
    rb.unwrap();

    assert_eq!(
        a.projection(),
        b.projection(),
        "converged over framed bytes"
    );
    assert_eq!(a.op_ids().len(), b.op_ids().len());
    assert_eq!(a.projection().groups.len(), 2);
}
