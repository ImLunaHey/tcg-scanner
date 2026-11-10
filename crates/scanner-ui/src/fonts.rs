//! Explicit fallback is necessary on the web, where system fonts are unavailable.
pub const FONTS: &[&[u8]] = &[
    include_bytes!("../web/Inter-Regular.ttf"),
    include_bytes!("../web/Inter-Medium.ttf"),
    include_bytes!("../web/Inter-SemiBold.ttf"),
    include_bytes!("../assets/NotoSansKR.ttf"),
];
