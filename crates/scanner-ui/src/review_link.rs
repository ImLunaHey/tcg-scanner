use rune::{
    El, text,
    tok::{self, type_},
};
pub(crate) fn validated(raw: &str) -> Option<String> {
    if raw.len() > 4096 {
        return None;
    }
    let url = url::Url::parse(raw).ok()?;
    (url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}
pub(crate) fn control(url: String, t: tok::Theme) -> El {
    let link = text("Review in Store")
        .key("store-review-link")
        .font(type_::LABEL)
        .color(t.text);
    #[cfg(not(target_os = "linux"))]
    {
        link.href(url)
    }
    #[cfg(target_os = "linux")]
    {
        link.role("button")
            .on_press::<crate::state::State>(move |s| {
                let url = url.clone();
                s.ops
                    .transfer
                    .link
                    .request(async move { open_portal(&url).await });
            })
    }
}
/// Use the desktop portal directly; Rune's Linux URL helper invokes xdg-open.
#[cfg(target_os = "linux")]
async fn open_portal(url: &str) -> scanner_types::error::Result<()> {
    let request = async {
        let connection = zbus::Connection::session().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.OpenURI",
        )
        .await?;
        let options = std::collections::HashMap::<&str, zbus::Variant<'_>>::new();
        proxy.call_method("OpenURI", &("", url, options)).await?;
        Ok::<_, zbus::Error>(())
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), request)
        .await
        .ok()
        .and_then(Result::ok)
        .ok_or_else(|| {
            scanner_types::error::ScannerError::new(
                "desktop",
                "Could not ask the desktop to open the Store review link.",
            )
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    const SAFE: &str = "https://store.example/review/1";
    #[test]
    fn review_links_require_https_and_never_contain_credentials() {
        assert_eq!(validated(SAFE).as_deref(), Some(SAFE));
        for value in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "http://store.example",
            "https://user:secret@store.example",
        ] {
            assert!(validated(value).is_none());
        }
    }
}
