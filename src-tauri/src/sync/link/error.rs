//! Why a link ends without linking.

use crate::sync::link::wire::AbortReason;

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    /// The other side's proof of the code did not verify, or came from an
    /// endpoint other than the connection's.
    #[error("the proof of the link code does not verify")]
    BadProof,
    /// The two sides' versions differ (FR-029).
    #[error("the versions of the two devices differ")]
    Incompatible,
    /// This device holds no vault secret, so it cannot add devices.
    #[error("this device is not a main device")]
    NotMainDevice,
    /// The other side ended the link on purpose.
    #[error("the other device ended the link: {0:?}")]
    Aborted(AbortReason),
    /// The user on the main device declined.
    #[error("the link was declined")]
    Declined,
    #[error("link protocol violation: {0}")]
    Protocol(&'static str),
    #[error("the new device list does not check out: {0}")]
    List(String),
    #[error(transparent)]
    Wire(#[from] crate::sync::wire::WireError),
    #[error(transparent)]
    Crdt(#[from] haex_crdt::Error),
    #[error(transparent)]
    Inbound(#[from] crate::sync::inbound::InboundError),
    #[error("a database task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("the link ran out of time")]
    TimedOut,
}
