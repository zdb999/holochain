use crate::actor::*;
use crate::event::*;

mod actor;
use actor::*;

/// Spawn a new KitsuneP2p actor.
pub async fn spawn_kitsune_p2p() -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    KitsuneP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let internal_sender = channel_factory.create_channel::<Internal>().await?;

    let sender = channel_factory.create_channel::<KitsuneP2p>().await?;

    tokio::task::spawn(builder.spawn(KitsuneP2pActor::new(
        channel_factory,
        internal_sender,
        evt_send,
    )?));

    Ok((sender, evt_recv))
}
