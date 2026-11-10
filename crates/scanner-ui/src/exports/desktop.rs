use super::Request;
use crate::state::State;
use rune::{FileDialogOptions, FileDialogResult, Tx};
use scanner_client::Client;
use scanner_types::error::Result;
use std::path::PathBuf;
pub(super) fn choose(s: &mut State, tx: &mut Tx, client: Client, request: Request) {
    s.exports.choosing = true;
    let folder = matches!(request, Request::Batch(_));
    let name = request.name();
    let extension = name.rsplit('.').next().unwrap_or("json");
    let options = FileDialogOptions {
        title: if folder {
            "Choose batch export folder".into()
        } else {
            "Export verified file".into()
        },
        file_name: Some(name.clone()),
        ..Default::default()
    }
    .filter("Export", &[extension]);
    let answer = move |s: &mut State, result: FileDialogResult, _: &mut Tx| {
        if !s.exports.choosing || s.client.is_none() {
            return;
        }
        s.exports.choosing = false;
        match result {
            Ok(Some(path)) => s
                .exports
                .task
                .request(async move { write(client, request, path).await }),
            Ok(None) => {}
            Err(error) => s.exports.notice = Some(error.to_string()),
        }
    };
    if folder {
        tx.open_folder(options, answer);
    } else {
        tx.save_file(options, answer);
    }
}
async fn write(client: Client, request: Request, path: PathBuf) -> Result<String> {
    match request {
        Request::Json { bytes, .. } => scanner_export::native::json(bytes, path.clone()).await?,
        Request::Image { job, entry } => {
            scanner_export::native::image(&client, job, &entry, path.clone()).await?
        }
        Request::Batch(batch) => {
            let root = scanner_export::native::batch(&client, &batch, path).await?;
            return Ok(format!("Batch exported to {}", root.display()));
        }
    }
    Ok(format!("Exported {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_dialog_cancellation_and_selected_destination_drive_the_export() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("manifest.json");
        let client = Client::new("http://127.0.0.1:8171", &"a".repeat(64)).unwrap();
        let mut state = State::default();
        state.client = Some(client.clone());
        let mut rt = rune::Runtime::new(
            state,
            |_, _| rune::el(),
            (300., 200.),
            rune::Env::new(false, 1.),
        );
        let request = || Request::Json {
            bytes: b"{\"fixture\":true}".to_vec(),
            name: "manifest.json".into(),
        };
        rt.update(|s, tx| choose(s, tx, client.clone(), request()));
        assert_eq!(rt.file_dialog().unwrap().0, rune::FileDialogKind::Save);
        rt.resolve_file_dialog(None).unwrap();
        rt.frame(0.);
        assert!(!rt.state.exports.busy());
        assert!(!path.exists());
        rt.update(|s, tx| choose(s, tx, client.clone(), request()));
        rt.resolve_file_dialog(Some(path.clone())).unwrap();
        rt.frame(0.016);
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !rt.state.poll_export() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"fixture\":true}");
        assert!(
            rt.state
                .exports
                .notice
                .as_ref()
                .unwrap()
                .starts_with("Exported ")
        );
        rt.update(|s, tx| choose(s, tx, client.clone(), request()));
        rt.state.exports.clear();
        rt.state.client = None;
        let cancelled = directory.path().join("cancelled.json");
        rt.resolve_file_dialog(Some(cancelled.clone())).unwrap();
        rt.frame(0.032);
        assert!(!rt.state.exports.busy());
        assert!(!cancelled.exists());
    }
}
