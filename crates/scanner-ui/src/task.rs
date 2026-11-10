//! One cancellable async read per UI slot. Replacing a receiver discards stale results.
use futures_channel::oneshot;
use futures_util::future::{AbortHandle, Abortable};
use rune::Waker;
use scanner_types::error::{Result, ScannerError};
use std::{future::Future, sync::Arc};

pub struct Task<T> {
    receiver: Option<oneshot::Receiver<Result<T>>>,
    abort: Option<AbortHandle>,
    ready: Option<Result<T>>,
    waker: Waker,
}
impl<T> Default for Task<T> {
    fn default() -> Self {
        Self {
            receiver: None,
            abort: None,
            ready: None,
            waker: Arc::new(|| {}),
        }
    }
}
impl<T> Task<T> {
    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = waker;
    }
    pub fn pending(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn cancel(&mut self) {
        if let Some(abort) = self.abort.take() {
            abort.abort();
        }
        self.receiver = None;
        self.ready = None;
    }
    pub fn take(&mut self) -> Option<Result<T>> {
        if let Some(result) = self.ready.take() {
            return Some(result);
        }
        let result = match self.receiver.as_mut()?.try_recv() {
            Ok(None) => return None,
            Ok(Some(result)) => result,
            Err(_) => Err(ScannerError::new(
                "task",
                "Background request ended unexpectedly.",
            )),
        };
        self.receiver = None;
        self.abort = None;
        Some(result)
    }
    fn prepare<F: Future<Output = Result<T>>>(
        &mut self,
        future: F,
    ) -> impl Future<Output = ()> + use<T, F> {
        self.cancel();
        let (sender, receiver) = oneshot::channel();
        let (abort, registration) = AbortHandle::new_pair();
        self.abort = Some(abort);
        self.receiver = Some(receiver);
        let wake = Wake(self.waker.clone());
        async move {
            let _wake = wake;
            if let Ok(result) = Abortable::new(future, registration).await {
                let _ = sender.send(result);
            }
        }
    }
}
struct Wake(Waker);
impl Drop for Wake {
    fn drop(&mut self) {
        (self.0)();
    }
}
impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + 'static> Task<T> {
    pub fn request(&mut self, future: impl Future<Output = Result<T>> + Send + 'static) {
        use std::sync::LazyLock;
        static RUNTIME: LazyLock<std::result::Result<tokio::runtime::Runtime, String>> =
            LazyLock::new(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())
            });
        match &*RUNTIME {
            Ok(runtime) => {
                runtime.spawn(self.prepare(future));
            }
            Err(error) => {
                self.cancel();
                self.ready = Some(Err(ScannerError::new("task", error)));
                (self.waker)();
            }
        }
    }
}
#[cfg(target_arch = "wasm32")]
impl<T: 'static> Task<T> {
    pub fn request(&mut self, future: impl Future<Output = Result<T>> + 'static) {
        wasm_bindgen_futures::spawn_local(self.prepare(future));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn superseded_or_cancelled_results_cannot_reach_the_current_selection() {
        let mut task = Task::<u32>::default();
        let old = task.prepare(async { Ok(1) });
        let current = task.prepare(async { Ok(2) });
        old.await;
        assert!(task.take().is_none());
        assert!(task.pending());
        current.await;
        assert_eq!(task.take().unwrap().unwrap(), 2);
        assert!(!task.pending());
        let cancelled = task.prepare(async { Ok(3) });
        task.cancel();
        cancelled.await;
        assert!(task.take().is_none());
    }
}
