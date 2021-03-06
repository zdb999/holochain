#![allow(clippy::too_many_arguments)]
//! Module containing incoming events from the HolochainP2p actor.

use crate::*;
use holochain_zome_types::signature::Signature;
use kitsune_p2p::agent_store::AgentInfoSigned;

/// Get options help control how the get is processed at various levels.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetOptions {
    /// Whether the remote-end should follow redirects or just return the
    /// requested entry.
    pub follow_redirects: bool,
    /// Return all live headers even if there is deletes.
    /// Useful for metadata calls.
    pub all_live_headers_with_metadata: bool,
}

impl From<&actor::GetOptions> for GetOptions {
    fn from(a: &actor::GetOptions) -> Self {
        Self {
            follow_redirects: a.follow_redirects,
            all_live_headers_with_metadata: a.all_live_headers_with_metadata,
        }
    }
}

/// GetMeta options help control how the get is processed at various levels.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetMetaOptions {}

impl From<&actor::GetMetaOptions> for GetMetaOptions {
    fn from(_a: &actor::GetMetaOptions) -> Self {
        Self {}
    }
}

/// GetLinks options help control how the get is processed at various levels.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetLinksOptions {}

impl From<&actor::GetLinksOptions> for GetLinksOptions {
    fn from(_a: &actor::GetLinksOptions) -> Self {
        Self {}
    }
}

ghost_actor::ghost_chan! {
    /// The HolochainP2pEvent stream allows handling events generated from
    /// the HolochainP2p actor.
    pub chan HolochainP2pEvent<super::HolochainP2pError> {
        /// We need to store signed agent info.
        fn put_agent_info_signed(dna_hash: DnaHash, to_agent: AgentPubKey, agent_info_signed: AgentInfoSigned) -> ();

        /// We need to get previously stored agent info.
        fn get_agent_info_signed(dna_hash: DnaHash, to_agent: AgentPubKey, kitsune_space: Arc<kitsune_p2p::KitsuneSpace>, kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>) -> Option<AgentInfoSigned>;

        /// A remote node is attempting to make a remote call on us.
        fn call_remote(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            from_agent: AgentPubKey,
            zome_name: ZomeName,
            fn_name: FunctionName,
            cap: Option<CapSecret>,
            request: SerializedBytes,
        ) -> SerializedBytes;

        /// A remote node is publishing data in a range we claim to be holding.
        fn publish(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            from_agent: AgentPubKey,
            request_validation_receipt: bool,
            dht_hash: holo_hash::AnyDhtHash,
            ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        ) -> ();

        /// A remote node is requesting a validation package.
        fn get_validation_package(
            // The dna_hash / space_hash context.
            dna_hash: DnaHash,
            // The agent_id / agent_pub_key context.
            to_agent: AgentPubKey,
            header_hash: HeaderHash,
        ) -> ValidationPackageResponse;

        /// A remote node is requesting entry data from us.
        fn get(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            dht_hash: holo_hash::AnyDhtHash,
            options: GetOptions,
        ) -> GetElementResponse;

        /// A remote node is requesting metadata from us.
        fn get_meta(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            dht_hash: holo_hash::AnyDhtHash,
            options: GetMetaOptions,
        ) -> MetadataSet;

        /// A remote node is requesting link data from us.
        fn get_links(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            link_key: WireLinkMetaKey,
            options: GetLinksOptions,
        ) -> GetLinksResponse;

        /// A remote node has sent us a validation receipt.
        fn validation_receipt_received(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            receipt: SerializedBytes,
        ) -> ();

        /// The p2p module wishes to query our DhtOpHash store.
        fn fetch_op_hashes_for_constraints(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            dht_arc: kitsune_p2p::dht_arc::DhtArc,
            since: holochain_types::Timestamp,
            until: holochain_types::Timestamp,
        ) -> Vec<holo_hash::DhtOpHash>;

        /// The p2p module needs access to the content for a given set of DhtOpHashes.
        fn fetch_op_hash_data(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            op_hashes: Vec<holo_hash::DhtOpHash>,
        ) -> Vec<(holo_hash::AnyDhtHash, holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>;

        /// P2p operations require cryptographic signatures and validation.
        fn sign_network_data(
            // The dna_hash / space_hash context.
            dna_hash: DnaHash,
            // The agent_id / agent_pub_key context.
            to_agent: AgentPubKey,
            // The data to sign.
            data: Vec<u8>,
        ) -> Signature;
    }
}

/// utility macro to make it more ergonomic to access the enum variants
macro_rules! match_p2p_evt {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            HolochainP2pEvent::CallRemote { $i, .. } => { $($t)* }
            HolochainP2pEvent::Publish { $i, .. } => { $($t)* }
            HolochainP2pEvent::GetValidationPackage { $i, .. } => { $($t)* }
            HolochainP2pEvent::Get { $i, .. } => { $($t)* }
            HolochainP2pEvent::GetMeta { $i, .. } => { $($t)* }
            HolochainP2pEvent::GetLinks { $i, .. } => { $($t)* }
            HolochainP2pEvent::ValidationReceiptReceived { $i, .. } => { $($t)* }
            HolochainP2pEvent::FetchOpHashesForConstraints { $i, .. } => { $($t)* }
            HolochainP2pEvent::FetchOpHashData { $i, .. } => { $($t)* }
            HolochainP2pEvent::SignNetworkData { $i, .. } => { $($t)* }
            HolochainP2pEvent::PutAgentInfoSigned { $i, .. } => { $($t)* }
            HolochainP2pEvent::GetAgentInfoSigned { $i, .. } => { $($t)* }
        }
    };
}

impl HolochainP2pEvent {
    /// The dna_hash associated with this network p2p event.
    pub fn dna_hash(&self) -> &DnaHash {
        match_p2p_evt!(self => |dna_hash| { dna_hash })
    }

    /// The agent_pub_key associated with this network p2p event.
    pub fn as_to_agent(&self) -> &AgentPubKey {
        match_p2p_evt!(self => |to_agent| { to_agent })
    }
}

/// Receiver type for incoming holochain p2p events.
pub type HolochainP2pEventReceiver = futures::channel::mpsc::Receiver<HolochainP2pEvent>;
