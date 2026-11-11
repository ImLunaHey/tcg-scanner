//! In-process YOLOv8 card localization. Recognition remains a separate step.
mod boxes;
use image::{
    RgbImage,
    imageops::{self, FilterType},
};
use ort::{
    environment::Environment,
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use scanner_types::{
    binder::BinderDetection,
    error::{Result, ScannerError},
};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct BinderDetector {
    session: Session,
    _environment: Environment,
}
impl BinderDetector {
    pub fn load(runtime: &Path, model: &Path, sha256: &str) -> Result<Arc<Mutex<Self>>> {
        let runtime = crate::model_runtime::ModelRuntime::load(runtime)?;
        Self::with_runtime(&runtime, model, sha256)
    }
    pub fn with_runtime(
        runtime: &crate::model_runtime::ModelRuntime,
        model: &Path,
        sha256: &str,
    ) -> Result<Arc<Mutex<Self>>> {
        if std::fs::metadata(model).map_err(error)?.len() > 32 * 1024 * 1024 {
            return Err(error("Binder model exceeds 32 MiB."));
        }
        let graph = std::fs::read(model).map_err(error)?;
        if format!("{:x}", Sha256::digest(&graph)) != sha256 {
            return Err(error("Binder model checksum mismatch."));
        }
        let environment = runtime.environment.clone();
        let session = Session::builder(&environment)
            .map_err(error)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(error)?
            .with_intra_threads(2)
            .map_err(error)?
            .commit_from_memory(&graph)
            .map_err(error)?;
        if session.inputs().len() != 1 || session.inputs()[0].name() != "images" {
            return Err(error("Expected YOLOv8 images input."));
        }
        Ok(Arc::new(Mutex::new(Self {
            session,
            _environment: environment,
        })))
    }
    pub fn detect(&mut self, bytes: &[u8]) -> Result<BinderDetection> {
        let image = scanner_images::camera::decode(bytes)?.to_rgb8();
        let (pixels, geometry) = letterbox(&image);
        let tensor = Tensor::from_array(([1_usize, 3, 640, 640], pixels)).map_err(error)?;
        let outputs = self
            .session
            .run(ort::inputs!["images" => tensor])
            .map_err(error)?;
        let output = outputs
            .get("output0")
            .ok_or_else(|| error("Missing YOLOv8 boxes."))?;
        let (shape, values) = output.try_extract_tensor::<f32>().map_err(error)?;
        if shape.as_ref() != [1, 5, 8400] {
            return Err(error("Expected one-class YOLOv8 output [1,5,8400]."));
        }
        Ok(BinderDetection {
            schema_version: 1,
            method: "yolov8-card-onnx-v1".into(),
            cards: boxes::decode(values, geometry),
        })
    }
}
#[derive(Clone, Copy)]
struct Geometry {
    width: f32,
    height: f32,
    scale: f32,
    left: f32,
    top: f32,
}
fn letterbox(image: &RgbImage) -> (Vec<f32>, Geometry) {
    let scale = 640. / image.width().max(image.height()) as f32;
    let width = (image.width() as f32 * scale).round() as u32;
    let height = (image.height() as f32 * scale).round() as u32;
    let (left, top) = ((640 - width) / 2, (640 - height) / 2);
    let resized = imageops::resize(image, width, height, FilterType::Triangle);
    let mut values = vec![114. / 255.; 3 * 640 * 640];
    for (x, y, pixel) in resized.enumerate_pixels() {
        let i = ((y + top) * 640 + x + left) as usize;
        for c in 0..3 {
            values[c * 640 * 640 + i] = f32::from(pixel[c]) / 255.;
        }
    }
    (
        values,
        Geometry {
            width: image.width() as f32,
            height: image.height() as f32,
            scale,
            left: left as f32,
            top: top as f32,
        },
    )
}
fn error(e: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognitionModel", e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn letterbox_preserves_aspect_and_rgb_channels() {
        let (pixels, g) = letterbox(&RgbImage::from_pixel(300, 600, image::Rgb([255, 0, 0])));
        assert_eq!(g.left, 160.);
        assert_eq!(g.top, 0.);
        assert_eq!(pixels[0], 114. / 255.);
        assert_eq!(pixels[160], 1.);
        assert_eq!(pixels[640 * 640 + 160], 0.);
    }
}
