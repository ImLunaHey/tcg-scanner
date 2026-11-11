use crate::configuration::{Configuration, config};
use scanner_core::NativeCardRecognizer;
use scanner_recognition::{
    binder::BinderDetector, embedding::VisualRetriever, model_runtime::ModelRuntime,
};
use scanner_types::error::Result;
use std::sync::Arc;

pub(super) async fn configure(
    mut recognizer: NativeCardRecognizer,
    configuration: &Configuration,
) -> Result<NativeCardRecognizer> {
    let visual = configuration.visual_recognition.clone();
    let binder = configuration.binder_detection.clone();
    tokio::task::spawn_blocking(move || {
        let Some(library) = visual
            .as_ref()
            .map(|v| &v.runtime_library)
            .or_else(|| binder.as_ref().map(|b| &b.runtime_library))
        else {
            return Ok(recognizer);
        };
        if let (Some(v), Some(b)) = (&visual, &binder)
            && v.runtime_library.canonicalize().map_err(config)?
                != b.runtime_library.canonicalize().map_err(config)?
        {
            return Err(config(
                "DINO and binder models must use the same ONNX Runtime library.",
            ));
        }
        let runtime = ModelRuntime::load(library)?;
        if let Some(v) = visual {
            recognizer = recognizer.with_visual(Arc::new(VisualRetriever::with_runtime(
                &runtime, &v.model, &v.index,
            )?));
        }
        if let Some(b) = binder {
            recognizer = recognizer
                .with_binder(BinderDetector::with_runtime(&runtime, &b.model, &b.sha256)?);
        }
        Ok(recognizer)
    })
    .await
    .map_err(config)?
}
