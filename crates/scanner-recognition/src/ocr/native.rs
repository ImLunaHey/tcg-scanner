use super::{
    ocr_error,
    tsv::{self, RecognizedLine},
};
use image::RgbImage;
use libloading::Library;
use scanner_types::error::Result;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_uchar, c_void},
    path::Path,
    ptr::NonNull,
};

type Handle = *mut c_void;
type Create = unsafe extern "C" fn() -> Handle;
type Delete = unsafe extern "C" fn(Handle);
type Init = unsafe extern "C" fn(
    Handle,
    *const c_char,
    *const c_char,
    c_int,
    *mut *mut c_char,
    c_int,
    *mut *mut c_char,
    *mut *mut c_char,
    usize,
    c_int,
) -> c_int;
type SetImage = unsafe extern "C" fn(Handle, *const c_uchar, c_int, c_int, c_int, c_int);
type Integer = unsafe extern "C" fn(Handle, c_int);
type Tsv = unsafe extern "C" fn(Handle, c_int) -> *mut c_char;
type FreeText = unsafe extern "C" fn(*const c_char);

struct Api {
    _library: Library,
    create: Create,
    delete: Delete,
    init: Init,
    image: SetImage,
    resolution: Integer,
    segmentation: Integer,
    tsv: Tsv,
    free_text: FreeText,
}

impl Api {
    fn load(path: &Path) -> Result<Self> {
        // SAFETY: caller explicitly selects a trusted native library. These symbols and
        // signatures are Tesseract's stable C API; Library outlives every copied pointer.
        unsafe {
            let library = Library::new(path).map_err(ocr_error)?;
            Ok(Self {
                create: *library.get(b"TessBaseAPICreate\0").map_err(ocr_error)?,
                delete: *library.get(b"TessBaseAPIDelete\0").map_err(ocr_error)?,
                init: *library.get(b"TessBaseAPIInit4\0").map_err(ocr_error)?,
                image: *library.get(b"TessBaseAPISetImage\0").map_err(ocr_error)?,
                resolution: *library
                    .get(b"TessBaseAPISetSourceResolution\0")
                    .map_err(ocr_error)?,
                segmentation: *library
                    .get(b"TessBaseAPISetPageSegMode\0")
                    .map_err(ocr_error)?,
                tsv: *library.get(b"TessBaseAPIGetTsvText\0").map_err(ocr_error)?,
                free_text: *library.get(b"TessDeleteText\0").map_err(ocr_error)?,
                _library: library,
            })
        }
    }
}

/// Own one engine per worker. The raw handle deliberately prevents cross-thread sharing.
pub struct Tesseract {
    api: Api,
    handle: NonNull<c_void>,
}

impl Tesseract {
    pub fn new(library: &Path, data: &Path, language: &str) -> Result<Self> {
        if !matches!(language, "en" | "ko") {
            return Err(ocr_error("Supported languages are en and ko."));
        }
        let data = CString::new(
            data.to_str()
                .ok_or_else(|| ocr_error("Tesseract data path must be UTF-8."))?,
        )
        .map_err(ocr_error)?;
        let api = Api::load(library)?;
        // SAFETY: loaded create function takes no arguments and returns an owned API handle.
        let handle = NonNull::new(unsafe { (api.create)() })
            .ok_or_else(|| ocr_error("Tesseract initialization failed."))?;
        let engine = Self { api, handle };
        let language = if language == "ko" { c"kor+eng" } else { c"eng" };
        // Dictionary loading is initialization-only; pass it before the model is loaded.
        let mut system = b"load_system_dawg\0".to_vec();
        let mut frequency = b"load_freq_dawg\0".to_vec();
        let mut no_system = b"0\0".to_vec();
        let mut no_frequency = b"0\0".to_vec();
        let mut names = [system.as_mut_ptr().cast(), frequency.as_mut_ptr().cast()];
        let mut values = [
            no_system.as_mut_ptr().cast(),
            no_frequency.as_mut_ptr().cast(),
        ];
        // SAFETY: owned live handle, valid mutable NUL-terminated buffers and array lengths.
        // LSTM engine mode = 1. Configs are empty; Tesseract copies settings during init.
        if unsafe {
            (engine.api.init)(
                engine.handle.as_ptr(),
                data.as_ptr(),
                language.as_ptr(),
                1,
                std::ptr::null_mut(),
                0,
                names.as_mut_ptr(),
                values.as_mut_ptr(),
                names.len(),
                0,
            )
        } != 0
        {
            return Err(ocr_error(
                "Tesseract could not load the requested trained data.",
            ));
        }
        Ok(engine)
    }

    pub fn recognize(
        &mut self,
        image: &RgbImage,
        single_block: bool,
    ) -> Result<Vec<RecognizedLine>> {
        self.recognize_with_words(image, single_block, false)
    }
    pub(crate) fn recognize_words(
        &mut self,
        image: &RgbImage,
        single_block: bool,
    ) -> Result<Vec<RecognizedLine>> {
        self.recognize_with_words(image, single_block, true)
    }
    fn recognize_with_words(
        &mut self,
        image: &RgbImage,
        single_block: bool,
        words: bool,
    ) -> Result<Vec<RecognizedLine>> {
        let (width, height) = image.dimensions();
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || u64::from(width) * u64::from(height) > 8_000_000
        {
            return Err(ocr_error(
                "OCR input exceeds 4096 pixels per side or 8 megapixels.",
            ));
        }
        // SAFETY: packed RGB buffer dimensions/stride agree with its allocation. It remains
        // live throughout recognition. Tesseract owns the returned text until free_text.
        unsafe {
            (self.api.image)(
                self.handle.as_ptr(),
                image.as_raw().as_ptr(),
                width as c_int,
                height as c_int,
                3,
                (width * 3) as c_int,
            );
            (self.api.resolution)(self.handle.as_ptr(), 300);
            (self.api.segmentation)(self.handle.as_ptr(), if single_block { 6 } else { 11 });
            let text = (self.api.tsv)(self.handle.as_ptr(), 0);
            if text.is_null() {
                return Err(ocr_error("Tesseract recognition failed."));
            }
            let text_value = CStr::from_ptr(text).to_string_lossy();
            let result = if words {
                tsv::parse_words(&text_value, width, height)
            } else {
                tsv::parse(&text_value, width, height)
            };
            (self.api.free_text)(text);
            Ok(result)
        }
    }
}

impl Drop for Tesseract {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner, and the library is still loaded.
        unsafe {
            (self.api.delete)(self.handle.as_ptr());
        }
    }
}
