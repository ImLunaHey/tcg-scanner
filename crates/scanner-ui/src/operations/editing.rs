use super::Command;
use crate::state::State;
use rune::TextEdit;
use scanner_types::{
    Extensions,
    error::{Result, ScannerError},
    images::{ImageCrop, ImageEditRequest},
};
use uuid::Uuid;

pub struct Editor {
    pub crop: [TextEdit; 4],
    pub deskew: TextEdit,
    pub turns: u8,
}
impl Editor {
    fn from_request(request: Option<&ImageEditRequest>) -> Self {
        let crop = request.map(|r| r.crop).unwrap_or_default();
        Self {
            crop: [crop.x, crop.y, crop.width, crop.height]
                .map(|v| TextEdit::new((v * 100.).to_string())),
            deskew: TextEdit::new(
                request
                    .and_then(|r| r.deskew_degrees)
                    .unwrap_or(0.)
                    .to_string(),
            ),
            turns: request.map(|r| r.quarter_turns).unwrap_or(0),
        }
    }
    fn request(&self, job_id: Uuid, sequence: u32) -> Result<ImageEditRequest> {
        let number = |edit: &TextEdit| -> Result<f64> {
            edit.text.trim().parse().map_err(|_| {
                ScannerError::new(
                    "invalidParams",
                    "Enter numbers for crop percentages and deskew degrees.",
                )
            })
        };
        let request = ImageEditRequest {
            id: Uuid::new_v4(),
            job_id,
            sequence,
            crop: ImageCrop {
                x: number(&self.crop[0])? / 100.,
                y: number(&self.crop[1])? / 100.,
                width: number(&self.crop[2])? / 100.,
                height: number(&self.crop[3])? / 100.,
            },
            quarter_turns: self.turns,
            deskew_degrees: Some(number(&self.deskew)?),
            extensions: Extensions::new(),
        };
        request.validate()?;
        Ok(request)
    }
}
impl State {
    pub fn begin_edit(&mut self) {
        if self.ops.busy() || self.fixture {
            return;
        }
        let (Some(record), Some(card)) = (&self.record, self.cards.get(self.selected)) else {
            return;
        };
        if !record.state.terminal() {
            return;
        }
        let request = record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .rev()
            .find(|e| e.request.sequence == card.sequence)
            .map(|e| &e.request);
        self.ops.editor = Some(Editor::from_request(request));
        self.error = None;
        self.load_selected_preview(true);
    }
    pub fn reset_edit(&mut self) {
        if !self.ops.busy() {
            self.ops.editor = Some(Editor::from_request(None));
        }
    }
    pub fn close_edit(&mut self) {
        if !self.ops.busy() {
            self.ops.editor = None;
            self.load_selected_preview(false);
        }
    }
    pub fn save_edit(&mut self) {
        if self.ops.busy() {
            return;
        }
        let (Some(record), Some(card), Some(editor)) = (
            &self.record,
            self.cards.get(self.selected),
            &self.ops.editor,
        ) else {
            return;
        };
        match editor.request(record.id, card.sequence) {
            Ok(request) => self.submit(Command {
                method: "images.edit",
                params: serde_json::json!(request),
                job: Some(request.job_id),
                focus: false,
            }),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_preserve_original_coordinates_and_reject_invalid_geometry() {
        let original = ImageEditRequest {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            sequence: 7,
            crop: ImageCrop {
                x: 0.1,
                y: 0.2,
                width: 0.8,
                height: 0.7,
            },
            quarter_turns: 3,
            deskew_degrees: Some(-2.5),
            extensions: Extensions::new(),
        };
        let mut editor = Editor::from_request(Some(&original));
        let output = editor.request(original.job_id, original.sequence).unwrap();
        assert_eq!(output.crop, original.crop);
        assert_eq!(output.quarter_turns, 3);
        assert_eq!(output.deskew_degrees, Some(-2.5));
        assert_ne!(output.id, original.id);
        editor.crop[2] = TextEdit::new("100");
        assert!(editor.request(original.job_id, 7).is_err());
        editor = Editor::from_request(None);
        editor.deskew = TextEdit::new("NaN");
        assert!(editor.request(original.job_id, 7).is_err());
        editor.deskew = TextEdit::new("13");
        assert!(editor.request(original.job_id, 7).is_err());
    }
}
