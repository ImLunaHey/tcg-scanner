use crate::ocr::Tesseract;
use scanner_types::{
    cards::{CardGame, CardLanguage},
    error::Result,
    recognition::RecognizedLine,
};
use std::path::Path;
#[derive(Clone)]
pub(crate) struct Reading {
    pub turns: u32,
    pub lines: Vec<RecognizedLine>,
}
#[derive(Clone, Copy, Default)]
pub(crate) struct Options {
    pub preferred: Option<u32>,
    pub deadline: Option<std::time::Instant>,
}
pub(crate) fn read_options(
    bytes: &[u8],
    library: &Path,
    data: &Path,
    language: CardLanguage,
    game: CardGame,
    options: Options,
) -> Result<Vec<Reading>> {
    let image = scanner_images::decode::decode(bytes)?.to_rgb8();
    let mut tesseract = Tesseract::new(
        library,
        data,
        match language {
            CardLanguage::English => "en",
            CardLanguage::Korean => "ko",
        },
    )?;
    if let Some(deadline) = options.deadline {
        tesseract.set_deadline(deadline);
    }
    let mut results = vec![];
    for turns in (0..4).filter(|t| options.preferred.is_none_or(|p| p == *t)) {
        let rotated = match turns {
            1 => image::imageops::rotate90(&image),
            2 => image::imageops::rotate180(&image),
            3 => image::imageops::rotate270(&image),
            _ => image.clone(),
        };
        let readings = tesseract.read_card(&rotated)?;
        if game == CardGame::Yugioh {
            results.push(Reading {
                turns,
                lines: tesseract.read_yugioh(&rotated)?,
            });
        }
        results.push(Reading {
            turns,
            lines: readings.regional(),
        });
        results.push(Reading {
            turns,
            lines: readings.whole,
        });
    }
    Ok(results)
}
