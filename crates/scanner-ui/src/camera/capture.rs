use scanner_types::error::{Result, ScannerError};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement, MediaStream, MediaStreamTrack,
};
#[derive(Clone, Default)]
pub struct Control {
    stopped: Rc<Cell<bool>>,
    stream: Rc<RefCell<Option<MediaStream>>>,
}
impl Control {
    pub fn stop(&self) {
        self.stopped.set(true);
        if let Some(stream) = self.stream.borrow().as_ref() {
            stop_stream(stream);
        }
    }
    pub fn stopped(&self) -> bool {
        self.stopped.get()
    }
}
struct Listener {
    target: web_sys::EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(web_sys::Event)>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}
pub struct Capture {
    control: Control,
    video: HtmlVideoElement,
    pub canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    _small: HtmlCanvasElement,
    small_context: CanvasRenderingContext2d,
    _listeners: Vec<Listener>,
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.control.stop();
        self.video.set_src_object(None);
        self.video.remove();
    }
}
pub struct Frame {
    pub sample: Vec<u8>,
    pub rgba: Vec<u8>,
}
impl Capture {
    pub async fn open(control: Control, wake: rune::Waker) -> Result<Self> {
        // This acquisition is owned independently of the UI receiver. If the
        // permission prompt resolves after Stop, its stream is stopped immediately.
        let (send, receive) = futures_channel::oneshot::channel();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = send.send(Self::acquire(control, wake).await);
        });
        receive
            .await
            .map_err(|_| error("Camera acquisition ended unexpectedly."))?
    }
    async fn acquire(control: Control, wake: rune::Waker) -> Result<Self> {
        let window = window()?;
        let document = document()?;
        if !window.is_secure_context() {
            return Err(error(
                "Camera access needs HTTPS. Open the scanner's secure Tailscale address.",
            ));
        }
        if !window.navigator().on_line() || document.hidden() {
            return Err(error(
                "Connect to the scanner service and keep this page visible before starting.",
            ));
        }
        let video: HtmlVideoElement = document
            .create_element("video")
            .map_err(js_error)?
            .dyn_into()
            .map_err(js_error)?;
        video.set_muted(true);
        video.set_attribute("playsinline", "").map_err(js_error)?;
        video
            .set_attribute("class", "camera-source")
            .map_err(js_error)?;
        video
            .set_attribute("aria-hidden", "true")
            .map_err(js_error)?;
        let (canvas, context) = create_canvas(1000, 1333)?;
        let (small, small_context) = create_canvas(240, 320)?;
        let capture = Self {
            video,
            canvas,
            context,
            _small: small,
            small_context,
            _listeners: listeners(control.clone(), wake)?,
            control,
        };
        document
            .body()
            .ok_or_else(|| error("Page is unavailable."))?
            .append_child(&capture.video)
            .map_err(js_error)?;
        let constraints = web_sys::MediaStreamConstraints::new();
        constraints.set_audio_bool(false);
        let video = web_sys::MediaTrackConstraints::new();
        video.set_facing_mode_str("environment");
        video.set_width_i32(1920);
        video.set_height_i32(1440);
        constraints.set_video_media_track_constraints(&video);
        let stream: MediaStream = JsFuture::from(
            window
                .navigator()
                .media_devices()
                .map_err(js_error)?
                .get_user_media_with_constraints(&constraints)
                .map_err(js_error)?,
        )
        .await
        .map_err(js_error)?
        .dyn_into()
        .map_err(js_error)?;
        if capture.control.stopped() {
            stop_stream(&stream);
            return Err(error("Camera stopped."));
        }
        // Copy the JS handle, not MediaStream.clone(), which creates new tracks.
        *capture.control.stream.borrow_mut() = Some(Clone::clone(&stream));
        capture.video.set_src_object(Some(&stream));
        JsFuture::from(capture.video.play().map_err(js_error)?)
            .await
            .map_err(js_error)?;
        if capture.control.stopped() {
            return Err(error("Camera stopped."));
        }
        Ok(capture)
    }
    pub fn frame(&self) -> Result<Option<Frame>> {
        let (w, h) = (
            f64::from(self.video.video_width()),
            f64::from(self.video.video_height()),
        );
        if self.video.ready_state() < 2 || w == 0. || h == 0. {
            return Ok(None);
        }
        let width = w.min(h * 1000. / 1333.);
        let height = h.min(w * 1333. / 1000.);
        self.context
            .draw_image_with_html_video_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &self.video,
                (w - width) / 2.,
                (h - height) / 2.,
                width,
                height,
                0.,
                0.,
                1000.,
                1333.,
            )
            .map_err(js_error)?;
        self.small_context
            .draw_image_with_html_canvas_element_and_dw_and_dh(&self.canvas, 0., 0., 240., 320.)
            .map_err(js_error)?;
        let rgba = self
            .small_context
            .get_image_data(0., 0., 240., 320.)
            .map_err(js_error)?
            .data()
            .0;
        let sample = (0..64)
            .flat_map(|y| (0..48).map(move |x| ((y * 4 + 32) * 240 + x * 4 + 24) * 4))
            .map(|i| {
                ((u32::from(rgba[i]) + u32::from(rgba[i + 1]) + u32::from(rgba[i + 2])) / 3) as u8
            })
            .collect();
        Ok(Some(Frame { sample, rgba }))
    }
    pub fn jpeg(&self) -> Result<String> {
        for quality in [0.82, 0.65, 0.45] {
            let encoded = self
                .canvas
                .to_data_url_with_type_and_encoder_options(
                    "image/jpeg",
                    &JsValue::from_f64(quality),
                )
                .map_err(js_error)?;
            if let Some(value) = encoded.strip_prefix("data:image/jpeg;base64,")
                && value.len() <= scanner_types::camera::MAX_IMAGE_BYTES.div_ceil(3) * 4
            {
                return Ok(value.into());
            }
        }
        Err(error(
            "Camera frame is too large. Try a simpler background.",
        ))
    }
    pub fn track(&self) -> Option<MediaStreamTrack> {
        self.control
            .stream
            .borrow()
            .as_ref()?
            .get_video_tracks()
            .get(0)
            .dyn_into()
            .ok()
    }
}
fn listeners(control: Control, wake: rune::Waker) -> Result<Vec<Listener>> {
    let mut listeners = vec![];
    for (target, name) in [
        (
            document()?.unchecked_into::<web_sys::EventTarget>(),
            "visibilitychange",
        ),
        (window()?.unchecked_into(), "offline"),
        (window()?.unchecked_into(), "pagehide"),
    ] {
        let control = control.clone();
        let wake = wake.clone();
        let callback = Closure::wrap(Box::new(move |_: web_sys::Event| {
            if name != "visibilitychange" || document().is_ok_and(|d| d.hidden()) {
                control.stop();
                wake();
            }
        }) as Box<dyn FnMut(_)>);
        target
            .add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())
            .map_err(js_error)?;
        listeners.push(Listener {
            target,
            name,
            callback,
        });
    }
    Ok(listeners)
}
fn stop_stream(stream: &MediaStream) {
    for value in stream.get_tracks().iter() {
        if let Ok(track) = value.dyn_into::<MediaStreamTrack>() {
            track.stop();
        }
    }
}
fn create_canvas(w: u32, h: u32) -> Result<(HtmlCanvasElement, CanvasRenderingContext2d)> {
    let canvas: HtmlCanvasElement = document()?
        .create_element("canvas")
        .map_err(js_error)?
        .dyn_into()
        .map_err(js_error)?;
    canvas.set_width(w);
    canvas.set_height(h);
    let context = canvas
        .get_context("2d")
        .map_err(js_error)?
        .ok_or_else(|| error("Camera canvas is unavailable."))?
        .dyn_into()
        .map_err(js_error)?;
    Ok((canvas, context))
}
fn window() -> Result<web_sys::Window> {
    web_sys::window().ok_or_else(|| error("Browser window is unavailable."))
}
fn document() -> Result<web_sys::Document> {
    window()?
        .document()
        .ok_or_else(|| error("Browser document is unavailable."))
}
pub fn error(message: &str) -> ScannerError {
    ScannerError::new("camera", message)
}
pub fn js_error(value: impl Into<JsValue>) -> ScannerError {
    let value = value.into();
    let name = js_sys::Reflect::get(&value, &"name".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    error(match name.as_str() {
        "NotAllowedError" => {
            "Camera access was denied. Allow camera access in browser settings, then try again."
        }
        "NotFoundError" => "No camera was found on this device.",
        "NotReadableError" => "The camera is in use by another app. Close it and try again.",
        _ => "The browser could not complete the camera operation.",
    })
}
