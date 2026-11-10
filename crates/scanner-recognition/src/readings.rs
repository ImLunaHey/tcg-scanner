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
pub(crate) fn read(
    bytes: Vec<u8>,
    library: &Path,
    data: &Path,
    language: CardLanguage,
) -> Result<Vec<Reading>> {
    read_game(bytes, library, data, language, CardGame::Pokemon)
}
pub(crate) fn read_game(
    bytes: Vec<u8>,
    library: &Path,
    data: &Path,
    language: CardLanguage,
    game: CardGame,
) -> Result<Vec<Reading>> {
    let image = scanner_images::decode::decode(&bytes)?.to_rgb8();
    let mut tesseract = Tesseract::new(
        library,
        data,
        match language {
            CardLanguage::English => "en",
            CardLanguage::Korean => "ko",
        },
    )?;
    let mut results = vec![];
    for turns in 0..4 {
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
