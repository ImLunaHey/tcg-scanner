use super::{Acknowledgement, Event, error};
use futures_util::StreamExt;
use scanner_types::error::Result;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use zbus::{Connection, OwnedFd, Proxy};

pub(super) async fn run(
    events: mpsc::UnboundedSender<Event>,
    stop: CancellationToken,
    ready: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let connection = Connection::system().await.map_err(error)?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await
    .map_err(error)?;
    let mut signals = proxy
        .receive_signal("PrepareForSleep")
        .await
        .map_err(error)?;
    let mut inhibitor = Some(inhibit(&proxy).await?);
    // Subscribe before reading state so startup during suspension cannot silently
    // admit scans. Duplicate sleep/wake events are harmless coordinator transitions.
    let sleeping: bool = proxy
        .get_property("PreparingForSleep")
        .await
        .map_err(error)?;
    if sleeping {
        send_sleep(&events, inhibitor.take());
    }
    let _ = ready.send(Ok(()));
    loop {
        tokio::select! {
            _ = stop.cancelled() => return Ok(()),
            signal = signals.next() => {
                let signal = signal.ok_or_else(|| error("Power notification stream disconnected."))?;
                let (sleeping,): (bool,) = signal.body().deserialize().map_err(error)?;
                if sleeping { send_sleep(&events, inhibitor.take()); }
                else {
                    inhibitor = Some(inhibit(&proxy).await?);
                    let _ = events.send(Event::Resume);
                }
            }
        }
    }
}
async fn inhibit(proxy: &Proxy<'_>) -> Result<OwnedFd> {
    proxy
        .call(
            "Inhibit",
            &(
                "sleep",
                "Scanner",
                "Close scanner sessions and preserve originals",
                "delay",
            ),
        )
        .await
        .map_err(error)
}
fn send_sleep(events: &mpsc::UnboundedSender<Event>, inhibitor: Option<OwnedFd>) {
    let _ = events.send(Event::Suspend(Acknowledgement::new(move || {
        drop(inhibitor)
    })));
}
