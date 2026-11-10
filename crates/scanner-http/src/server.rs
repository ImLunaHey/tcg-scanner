use crate::Gateway;
use hyper::server::conn::http1;
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};
use scanner_types::error::{Result, ScannerError};
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Semaphore, task::JoinSet};
use tokio_util::sync::CancellationToken;

impl Gateway {
    /// Serve HTTP/1 on the validated address, with bounded connections and header
    /// reads. The caller must keep the Engine/native host alive through shutdown.
    pub async fn serve(
        self: Arc<Self>,
        listener: TcpListener,
        stop: CancellationToken,
    ) -> Result<()> {
        if listener.local_addr().map_err(transport)? != self.address() {
            return Err(ScannerError::new(
                "configuration",
                "Listener does not match the validated gateway address.",
            ));
        }
        let slots = Arc::new(Semaphore::new(64));
        let mut connections = JoinSet::new();
        let outcome = loop {
            tokio::select! {
                biased;
                _=stop.cancelled()=>break Ok(()),
                Some(_)=connections.join_next(),if !connections.is_empty()=>{},
                accepted=listener.accept()=>{
                    let (stream,_)=match accepted {Ok(value)=>value,Err(error)=>break Err(transport(error))};
                    let Ok(permit)=slots.clone().try_acquire_owned() else {drop(stream);continue;};
                    let app=self.router();let shutdown=stop.clone();
                    connections.spawn(async move {
                        let _permit=permit;
                        let mut builder=http1::Builder::new();
                        builder.timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(10))
                            .max_headers(64).max_header_size(32768).max_buf_size(32768).keep_alive(false);
                        let connection=builder.serve_connection(TokioIo::new(stream),TowerToHyperService::new(app));
                        tokio::pin!(connection);
                        tokio::select! {
                            _=&mut connection=>{},
                            _=shutdown.cancelled()=>{
                                connection.as_mut().graceful_shutdown();
                                let _=tokio::time::timeout(Duration::from_secs(30),connection).await;
                            },
                            _=tokio::time::sleep(Duration::from_secs(305))=>{},
                        }
                    });
                }
            }
        };
        stop.cancel();
        drop(listener);
        while connections.join_next().await.is_some() {}
        outcome
    }
}
fn transport(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("transport", error.to_string())
}
