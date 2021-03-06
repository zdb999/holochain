#![deny(missing_docs)]

//! Defines [ConductorHandle], a lightweight cloneable reference to a Conductor
//! with a limited public interface.
//!
//! A ConductorHandle can be produced via [Conductor::into_handle]
//!
//! ```rust, no_run
//! async fn async_main () {
//! use holochain_state::test_utils::{test_conductor_env, test_wasm_env, test_p2p_env, TestEnvironment};
//! use holochain::conductor::{Conductor, ConductorBuilder, ConductorHandle};
//! let env = test_conductor_env();
//! let TestEnvironment {
//!  env: wasm_env,
//!  tmpdir: _tmpdir,
//! } = test_wasm_env();
//! let TestEnvironment {
//!  env: p2p_env,
//!  tmpdir: _p2p_tmpdir,
//! } = test_p2p_env();
//! let handle: ConductorHandle = ConductorBuilder::new()
//!    .test(env, wasm_env, p2p_env)
//!    .await
//!    .unwrap();
//!
//! // handles are cloneable
//! let handle2 = handle.clone();
//!
//! assert_eq!(handle.list_dnas().await, Ok(vec![]));
//! handle.shutdown().await;
//!
//! // handle2 will only get errors from now on, since the other handle
//! // shut down the conductor.
//! assert!(handle2.list_dnas().await.is_err());
//!
//! # }
//! ```
//!
//! The purpose of this handle is twofold:
//!
//! First, it specifies how to synchronize
//! read/write access to a single Conductor across multiple references. The various
//! Conductor APIs - [CellConductorApi], [AdminInterfaceApi], and [AppInterfaceApi],
//! use a ConductorHandle as their sole method of interaction with a Conductor.
//!
//! Secondly, it hides the concrete type of the Conductor behind a dyn Trait.
//! The Conductor is a central point of configuration, and has several
//! type parameters, used to modify functionality including specifying mock
//! types for testing. If we did not have a way of hiding this type genericity,
//! code which interacted with the Conductor would also have to be highly generic.

use super::{
    api::error::ConductorApiResult,
    config::AdminInterfaceConfig,
    dna_store::DnaStore,
    entry_def_store::EntryDefBufferKey,
    error::{ConductorResult, CreateAppError},
    interface::SignalBroadcaster,
    manager::TaskManagerRunHandle,
    Cell, Conductor,
};
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::ZomeCallInvocationResult;
use derive_more::From;
use holochain_types::{
    app::{AppId, InstalledApp, InstalledCell, MembraneProof},
    autonomic::AutonomicCue,
    cell::CellId,
    dna::DnaFile,
    prelude::*,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::*;

use futures::future::FutureExt;
use holochain_p2p::event::HolochainP2pEvent::GetAgentInfoSigned;
use holochain_p2p::event::HolochainP2pEvent::PutAgentInfoSigned;

#[cfg(test)]
use super::state::ConductorState;
#[cfg(test)]
use crate::core::queue_consumer::InitialQueueTriggers;
#[cfg(test)]
use holochain_state::env::EnvironmentWrite;
use holochain_zome_types::entry_def::EntryDef;

/// A handle to the Conductor that can easily be passed around and cheaply cloned
pub type ConductorHandle = Arc<dyn ConductorHandleT>;

/// Base trait for ConductorHandle
#[mockall::automock]
#[async_trait::async_trait]
pub trait ConductorHandleT: Send + Sync {
    /// Returns error if conductor is shutting down
    async fn check_running(&self) -> ConductorResult<()>;

    /// Add a collection of Admin interfaces and spawn the necessary tasks.
    ///
    /// This requires a concrete ConductorHandle to be passed into the
    /// interface tasks. This is a bit weird to do, but it was the only way
    /// around having a circular reference in the types.
    ///
    /// Never use a ConductorHandle for different Conductor here!
    #[allow(clippy::ptr_arg)]
    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>;

    /// Add an app interface
    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16>;

    /// Install a [Dna] in this Conductor
    async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()>;

    /// Get the list of hashes of installed Dnas in this Conductor
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;

    /// Get a [Dna] from the [DnaStore]
    async fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile>;

    /// Get a [EntryDef] from the [EntryDefBuffer]
    async fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef>;

    /// Add the [DnaFile]s from the wasm and dna_def databases into memory
    async fn add_dnas(&self) -> ConductorResult<()>;

    /// Dispatch a network event to the correct cell.
    async fn dispatch_holochain_p2p_event(
        &self,
        cell_id: &CellId,
        event: holochain_p2p::event::HolochainP2pEvent,
    ) -> ConductorResult<()>;

    /// Invoke a zome function on a Cell
    async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult>;

    /// Cue the autonomic system to perform some action early (experimental)
    async fn autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()>;

    /// Get a Websocket port which will
    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16>;

    /// Return the JoinHandle for all managed tasks, which when resolved will
    /// signal that the Conductor has completely shut down.
    ///
    /// NB: The JoinHandle is not cloneable,
    /// so this can only ever be called successfully once.
    async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle>;

    /// Send a signal to all managed tasks asking them to end ASAP.
    async fn shutdown(&self);

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &KeystoreSender;

    /// Request access to this conductor's networking handle
    fn holochain_p2p(&self) -> &holochain_p2p::HolochainP2pRef;

    /// Install Cells into ConductorState based on installation info, and run
    /// genesis on all new source chains
    #[allow(clippy::ptr_arg)]
    async fn install_app(
        self: Arc<Self>,
        app_id: AppId,
        cell_data_with_proofs: Vec<(InstalledCell, Option<MembraneProof>)>,
    ) -> ConductorResult<()>;

    /// Setup the cells from the database
    /// Only creates any cells that are not already created
    async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>>;

    /// Activate an app
    #[allow(clippy::ptr_arg)]
    async fn activate_app(&self, app_id: AppId) -> ConductorResult<()>;

    /// Deactivate an app
    #[allow(clippy::ptr_arg)]
    async fn deactivate_app(&self, app_id: AppId) -> ConductorResult<()>;

    /// List Cell Ids
    async fn list_cell_ids(&self) -> ConductorResult<Vec<CellId>>;

    /// List Active AppIds
    async fn list_active_app_ids(&self) -> ConductorResult<Vec<AppId>>;

    /// Dump the cells state
    #[allow(clippy::ptr_arg)]
    async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String>;

    /// Access the broadcast Sender which will send a Signal across every
    /// attached app interface
    async fn signal_broadcaster(&self) -> SignalBroadcaster;

    /// Get info about an installed App, whether active or inactive
    #[allow(clippy::ptr_arg)]
    async fn get_app_info(&self, app_id: &AppId) -> ConductorResult<Option<InstalledApp>>;

    #[cfg(test)]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite>;

    #[cfg(test)]
    async fn get_cell_triggers(&self, cell_id: &CellId)
        -> ConductorApiResult<InitialQueueTriggers>;

    // HACK: remove when B-01593 lands
    #[cfg(test)]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState>;
}

/// The current "production" implementation of a ConductorHandle.
/// The implementation specifies how read/write access to the Conductor
/// should be synchronized across multiple concurrent Handles.
///
/// Synchronization is currently achieved via a simple RwLock, but
/// this could be swapped out with, e.g. a channel Sender/Receiver pair
/// using an actor model.
#[derive(From)]
pub struct ConductorHandleImpl<DS: DnaStore + 'static> {
    pub(crate) conductor: RwLock<Conductor<DS>>,
    pub(crate) keystore: KeystoreSender,
    pub(crate) holochain_p2p: holochain_p2p::HolochainP2pRef,
}

#[async_trait::async_trait]
impl<DS: DnaStore + 'static> ConductorHandleT for ConductorHandleImpl<DS> {
    /// Check that shutdown has not been called
    async fn check_running(&self) -> ConductorResult<()> {
        self.conductor.read().await.check_running()
    }

    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        let mut lock = self.conductor.write().await;
        lock.add_admin_interfaces_via_handle(configs, self.clone())
            .await
    }

    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16> {
        let mut lock = self.conductor.write().await;
        lock.add_app_interface_via_handle(port, self.clone()).await
    }

    async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()> {
        let entry_defs = self.conductor.read().await.put_wasm(dna.clone()).await?;
        let mut store = self.conductor.write().await;
        store.dna_store_mut().add(dna);
        store.dna_store_mut().add_entry_defs(entry_defs);
        Ok(())
    }

    async fn add_dnas(&self) -> ConductorResult<()> {
        let (dnas, entry_defs) = self
            .conductor
            .read()
            .await
            .load_wasms_into_dna_files()
            .await?;
        let mut store = self.conductor.write().await;
        store.dna_store_mut().add_dnas(dnas);
        store.dna_store_mut().add_entry_defs(entry_defs);
        Ok(())
    }

    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
        Ok(self.conductor.read().await.dna_store().list())
    }

    async fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.conductor.read().await.dna_store().get(hash)
    }

    async fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
        self.conductor.read().await.dna_store().get_entry_def(key)
    }

    #[instrument(skip(self))]
    /// Warning: returning an error from this function kills the network for the conductor.
    async fn dispatch_holochain_p2p_event(
        &self,
        cell_id: &CellId,
        event: holochain_p2p::event::HolochainP2pEvent,
    ) -> ConductorResult<()> {
        let lock = self.conductor.read().await;
        match event {
            PutAgentInfoSigned {
                agent_info_signed,
                respond,
                ..
            } => {
                let res = lock
                    .put_agent_info_signed(agent_info_signed)
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            GetAgentInfoSigned {
                kitsune_space,
                kitsune_agent,
                respond,
                ..
            } => {
                let res = lock
                    .get_agent_info_signed(kitsune_space, kitsune_agent)
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            _ => {
                let cell: &Cell = lock.cell_by_id(cell_id)?;
                trace!(agent = ?cell_id.agent_pubkey(), event = ?event);
                cell.handle_holochain_p2p_event(event).await?;
            }
        }
        Ok(())
    }

    async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult> {
        // FIXME: D-01058: We are holding this read lock for
        // the entire call to call_zome and blocking
        // any writes to the conductor
        let lock = self.conductor.read().await;
        debug!(cell_id = ?invocation.cell_id);
        let cell: &Cell = lock.cell_by_id(&invocation.cell_id)?;
        Ok(cell.call_zome(invocation).await?)
    }

    async fn autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()> {
        let lock = self.conductor.write().await;
        let cell = lock.cell_by_id(cell_id)?;
        let _ = cell.handle_autonomic_process(cue.into()).await;
        Ok(())
    }

    async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
        self.conductor.write().await.take_shutdown_handle()
    }

    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.conductor
            .read()
            .await
            .get_arbitrary_admin_websocket_port()
    }

    async fn shutdown(&self) {
        self.conductor.write().await.shutdown()
    }

    fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    fn holochain_p2p(&self) -> &holochain_p2p::HolochainP2pRef {
        &self.holochain_p2p
    }

    async fn install_app(
        self: Arc<Self>,
        app_id: AppId,
        cell_data: Vec<(InstalledCell, Option<MembraneProof>)>,
    ) -> ConductorResult<()> {
        self.conductor
            .read()
            .await
            .genesis_cells(
                cell_data
                    .iter()
                    .map(|(c, p)| (c.as_id().clone(), p.clone()))
                    .collect(),
                self.clone(),
            )
            .await?;

        let cell_data = cell_data.into_iter().map(|(c, _)| c).collect();
        let app = InstalledApp { app_id, cell_data };

        // Update the db
        self.conductor
            .write()
            .await
            .add_inactive_app_to_db(app)
            .await
    }

    async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>> {
        let cells = {
            let lock = self.conductor.read().await;
            lock.create_active_app_cells(self.clone())
                .await?
                .into_iter()
        };
        let add_cells_tasks = cells.map(|result| async {
            match result {
                Ok(cells) => {
                    self.conductor.write().await.add_cells(cells);
                    None
                }
                Err(e) => Some(e),
            }
        });
        let r = futures::future::join_all(add_cells_tasks)
            .await
            .into_iter()
            // Remove successful and collect the errors
            .filter_map(|r| r)
            .collect();
        {
            self.conductor.write().await.initialize_cell_workflows();
        }
        Ok(r)
    }

    async fn activate_app(&self, app_id: AppId) -> ConductorResult<()> {
        self.conductor
            .write()
            .await
            .activate_app_in_db(app_id)
            .await
    }

    async fn deactivate_app(&self, app_id: AppId) -> ConductorResult<()> {
        let cell_ids_to_remove = self
            .conductor
            .write()
            .await
            .deactivate_app_in_db(app_id)
            .await?;
        self.conductor
            .write()
            .await
            .remove_cells(cell_ids_to_remove);
        Ok(())
    }

    async fn list_cell_ids(&self) -> ConductorResult<Vec<CellId>> {
        self.conductor.read().await.list_cell_ids().await
    }

    async fn list_active_app_ids(&self) -> ConductorResult<Vec<AppId>> {
        self.conductor.read().await.list_active_app_ids().await
    }

    async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
        self.conductor.read().await.dump_cell_state(cell_id).await
    }

    async fn signal_broadcaster(&self) -> SignalBroadcaster {
        self.conductor.read().await.signal_broadcaster()
    }

    async fn get_app_info(&self, app_id: &AppId) -> ConductorResult<Option<InstalledApp>> {
        Ok(self
            .conductor
            .read()
            .await
            .get_state()
            .await?
            .get_app_info(app_id))
    }

    #[cfg(test)]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite> {
        let lock = self.conductor.read().await;
        let cell = lock.cell_by_id(cell_id)?;
        Ok(cell.env().clone())
    }

    #[cfg(test)]
    async fn get_cell_triggers(
        &self,
        cell_id: &CellId,
    ) -> ConductorApiResult<InitialQueueTriggers> {
        let lock = self.conductor.read().await;
        let cell = lock.cell_by_id(cell_id)?;
        Ok(cell.triggers().clone())
    }

    #[cfg(test)]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState> {
        let lock = self.conductor.read().await;
        Ok(lock.get_state_from_handle().await?)
    }
}
