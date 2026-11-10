use super::{Request, error};
use scanner_client::Client;
use scanner_types::error::Result;
use wasm_bindgen::JsCast;
pub struct Artifact {
    name: String,
    media_type: &'static str,
    bytes: Vec<u8>,
}
pub(super) async fn prepare(client: Client, request: Request) -> Result<Artifact> {
    let name = request.name();
    let (media_type, bytes) = match request {
        Request::Json { bytes, .. } => {
            if bytes.len() > 16 * 1024 * 1024 {
                return Err(error("JSON export exceeds 16 MiB."));
            }
            ("application/json", bytes)
        }
        Request::Image { job, entry } => {
            let media = if entry.image.uniform_type == "public.png" {
                "image/png"
            } else {
                "image/tiff"
            };
            (
                media,
                scanner_export::image_bytes(&client, job, &entry).await?,
            )
        }
        Request::Batch(batch) => (
            "application/zip",
            scanner_export::archive(&client, &batch).await?,
        ),
    };
    Ok(Artifact {
        name,
        media_type,
        bytes,
    })
}
pub(super) fn save(artifact: Artifact) -> Result<String> {
    let failed = || error("Browser could not save the verified download.");
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(failed)?;
    let array = js_sys::Array::new();
    array.push(&js_sys::Uint8Array::from(artifact.bytes.as_slice()));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(artifact.media_type);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&array, &options)
        .map_err(|_| failed())?;
    let anchor = document
        .create_element("a")
        .map_err(|_| failed())?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| failed())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| failed())?;
    anchor.set_href(&url);
    anchor.set_download(&artifact.name);
    anchor.click();
    wasm_bindgen_futures::spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(30_000).await;
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    Ok(format!("Download ready: {}", artifact.name))
}
