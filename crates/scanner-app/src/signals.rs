use scanner_types::error::Result;

pub(crate) async fn shutdown() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate =
            signal(SignalKind::terminate()).map_err(crate::configuration::config)?;
        tokio::select! {_=terminate.recv()=>{},result=tokio::signal::ctrl_c()=>result.map_err(crate::configuration::config)?,}
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .map_err(crate::configuration::config)?;
    Ok(())
}
