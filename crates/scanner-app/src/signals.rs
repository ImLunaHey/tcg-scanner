use scanner_types::error::Result;

pub(crate) async fn shutdown() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate =
            signal(SignalKind::terminate()).map_err(crate::configuration::config)?;
        tokio::select! {_=terminate.recv()=>{},result=tokio::signal::ctrl_c()=>result.map_err(crate::configuration::config)?,}
    }
    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_logoff, ctrl_shutdown};
        let mut close = ctrl_close().map_err(crate::configuration::config)?;
        let mut logoff = ctrl_logoff().map_err(crate::configuration::config)?;
        let mut shutdown = ctrl_shutdown().map_err(crate::configuration::config)?;
        let mut interrupt = ctrl_break().map_err(crate::configuration::config)?;
        tokio::select! {
            _ = close.recv() => {}, _ = logoff.recv() => {}, _ = shutdown.recv() => {},
            _ = interrupt.recv() => {},
            result = tokio::signal::ctrl_c() => result.map_err(crate::configuration::config)?,
        }
    }
    #[cfg(not(any(unix, windows)))]
    tokio::signal::ctrl_c()
        .await
        .map_err(crate::configuration::config)?;
    Ok(())
}
