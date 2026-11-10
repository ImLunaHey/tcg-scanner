use super::{model_error, normalize};
use image::{
    RgbImage,
    imageops::{self, FilterType},
};
use ort::{
    environment::Environment,
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use scanner_types::error::Result;
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

pub const MODEL_REVISION: &str = "48988dfe73065df8d6f5ccc0edc7c8bcf307de41";
const MODEL_HASH: &str = "bb75e9e30ff382ecdbd150266445ab41272be4acc85fd3563218dd89781e36da";
const WEIGHTS_HASH: &str = "1eff0bb9f4fdef831ca61c8bc2b5c88d8cc21ef4756d3b483d9b9c15d8d5d27f";
const MODEL_KEY: &str =
    "dinov3-vits16:48988dfe73065df8d6f5ccc0edc7c8bcf307de41:srgb224-triangle-cls-v1";

pub struct DinoEncoder {
    session: Session,
    _environment: Environment,
}

impl DinoEncoder {
    /// Both graph and external weights are pinned; arbitrary unverified graphs are rejected.
    pub fn load(runtime: &Path, model: &Path) -> Result<Self> {
        let runtime = crate::model_runtime::ModelRuntime::load(runtime)?;
        Self::with_runtime(&runtime, model)
    }
    pub fn with_runtime(
        runtime: &crate::model_runtime::ModelRuntime,
        model: &Path,
    ) -> Result<Self> {
        verify_file(model, MODEL_HASH)?;
        verify_file(&model.with_file_name("model.onnx_data"), WEIGHTS_HASH)?;
        let environment = runtime.environment.clone();
        let session = Session::builder(&environment)
            .map_err(model_error)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(model_error)?
            .with_intra_threads(2)
            .map_err(model_error)?
            .commit_from_file(model)
            .map_err(model_error)?;
        if session.inputs().len() != 1 || session.inputs()[0].name() != "pixel_values" {
            return Err(model_error("Unexpected DINOv3 input signature."));
        }
        Ok(Self {
            session,
            _environment: environment,
        })
    }
    pub fn model_key(&self) -> &'static str {
        MODEL_KEY
    }

    pub fn encode(&mut self, image: &RgbImage) -> Result<Vec<f32>> {
        if image.width() == 0 || image.height() == 0 {
            return Err(model_error("Empty image."));
        }
        let pixels = preprocess(image);
        let tensor = Tensor::from_array(([1_usize, 3, 224, 224], pixels)).map_err(model_error)?;
        let output = self
            .session
            .run(ort::inputs!["pixel_values"=>tensor])
            .map_err(model_error)?;
        let value = output
            .get("last_hidden_state")
            .ok_or_else(|| model_error("Missing DINOv3 descriptor."))?;
        let (shape, values) = value.try_extract_tensor::<f32>().map_err(model_error)?;
        if shape.len() != 3 || shape[0] != 1 || shape[2] != 384 || values.len() < 384 {
            return Err(model_error("Invalid DINOv3 output shape."));
        }
        // First token is CLS. The next four are register tokens, not image descriptors.
        let mut cls = values[..384].to_vec();
        normalize(&mut cls)?;
        Ok(cls)
    }
}

fn preprocess(image: &RgbImage) -> Vec<f32> {
    let image = imageops::resize(image, 224, 224, FilterType::Triangle);
    let mean = [0.485_f32, 0.456, 0.406];
    let std = [0.229_f32, 0.224, 0.225];
    let mut output = vec![0_f32; 3 * 224 * 224];
    for (i, pixel) in image.pixels().enumerate() {
        for channel in 0..3 {
            output[channel * 224 * 224 + i] =
                (f32::from(pixel[channel]) / 255. - mean[channel]) / std[channel];
        }
    }
    output
}

fn verify_file(path: &Path, expected: &str) -> Result<()> {
    let mut file = std::fs::File::open(path).map_err(model_error)?;
    if file.metadata().map_err(model_error)?.len() > 512 * 1024 * 1024 {
        return Err(model_error("Model exceeds size limit."));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(model_error)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err(model_error(
            "Model checksum does not match the pinned revision.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preprocessing_is_rgb_channels_first_with_model_normalization() {
        let input = RgbImage::from_pixel(100, 200, image::Rgb([255, 0, 128]));
        let output = preprocess(&input);
        assert_eq!(output.len(), 3 * 224 * 224);
        assert!((output[0] - (1. - 0.485) / 0.229).abs() < 1e-5);
        assert!((output[224 * 224] + 0.456 / 0.224).abs() < 1e-5);
    }
}
