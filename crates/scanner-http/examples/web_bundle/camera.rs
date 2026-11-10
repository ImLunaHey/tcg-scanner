use base64::Engine as _;
use image::{Rgba, RgbaImage};
use std::{error::Error, path::Path};

pub fn write(output: &Path) -> Result<(), Box<dyn Error>> {
    let wasm = std::fs::read(output.join("scanner_worker_bg.wasm"))?;
    if wasm.len() > 1024 * 1024 || !wasm.starts_with(b"\0asm\x01\0\0\0") {
        return Err("Invalid camera worker WebAssembly module".into());
    }
    // Service worker dependencies are browser-managed installed scripts. Inline
    // WASM permits synchronous listener registration on every offline startup;
    // it never needs a separate network fetch or CacheStorage entry.
    let encoded = base64::engine::general_purpose::STANDARD.encode(wasm);
    let worker = format!(
        "import {{ initSync }} from './scanner_worker.js';\ninitSync({{ module: Uint8Array.from(atob('{encoded}'), c => c.charCodeAt(0)) }});\n"
    );
    std::fs::write(output.join("camera-worker.js"), worker)?;
    let manifest = serde_json::json!({
        "id":"/camera", "name":"Scanner card camera", "short_name":"Scanner",
        "start_url":"/camera", "scope":"/camera", "display":"standalone",
        "background_color":"#fafafa", "theme_color":"#18181b",
        "description":"Identify cards using your scanner service. Frames are not saved.",
        "icons":[
            {"src":"/camera-192.png","sizes":"192x192","type":"image/png","purpose":"any maskable"},
            {"src":"/camera-512.png","sizes":"512x512","type":"image/png","purpose":"any maskable"}
        ]
    });
    std::fs::write(
        output.join("camera.webmanifest"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    for size in [192, 512] {
        icon(size).save(output.join(format!("camera-{size}.png")))?;
    }
    Ok(())
}
fn icon(size: u32) -> RgbaImage {
    RgbaImage::from_fn(size, size, |x, y| {
        let x = x as f32 / size as f32;
        let y = y as f32 / size as f32;
        let colour = if (0.3..0.7).contains(&x) && (0.22..0.78).contains(&y) {
            if (0.34..0.66).contains(&x) && (0.3..0.6).contains(&y) {
                [72, 102, 211, 255]
            } else if (0.34..0.6).contains(&x) && (0.67..0.7).contains(&y) {
                [120, 120, 130, 255]
            } else {
                [250, 250, 250, 255]
            }
        } else {
            [24, 24, 27, 255]
        };
        Rgba(colour)
    })
}
