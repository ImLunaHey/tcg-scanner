//! Opt-in real-runtime integration: both sessions must coexist in one process.
use scanner_recognition::{
    binder::BinderDetector, embedding::DinoEncoder, model_runtime::ModelRuntime,
};
use std::path::Path;
#[test]
fn dino_and_binder_share_one_native_environment() {
    let Some(path) = std::env::var_os("SCANNER_MODEL_TEST_CONFIG") else {
        return;
    };
    let config: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let visual = &config["visualRecognition"];
    let binder = &config["binderDetection"];
    let runtime =
        ModelRuntime::load(Path::new(visual["runtimeLibrary"].as_str().unwrap())).unwrap();
    let mut dino =
        DinoEncoder::with_runtime(&runtime, Path::new(visual["model"].as_str().unwrap())).unwrap();
    let detector = BinderDetector::with_runtime(
        &runtime,
        Path::new(binder["model"].as_str().unwrap()),
        binder["sha256"].as_str().unwrap(),
    )
    .unwrap();
    let image = image::RgbImage::from_pixel(600, 800, image::Rgb([80, 90, 100]));
    assert_eq!(dino.encode(&image).unwrap().len(), 384);
    let mut jpeg = vec![];
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .encode_image(&image)
        .unwrap();
    assert_eq!(
        detector.lock().unwrap().detect(&jpeg).unwrap().method,
        "yolov8-card-onnx-v1"
    );
    assert_eq!(dino.encode(&image).unwrap().len(), 384);
}
