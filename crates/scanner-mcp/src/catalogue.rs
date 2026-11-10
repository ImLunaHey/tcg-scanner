use crate::{field::Field as F, tool::Tool as T};
use serde_json::json;
pub(crate) fn tools() -> Vec<T> {
    let device = vec![("deviceID", F::text(512))];
    let id = vec![("id", F::uuid().required())];
    let page = vec![
        ("limit", F::integer(1, 100)),
        ("offset", F::integer(0, 1_000_000)),
    ];
    let image = vec![
        ("jobID", F::uuid().required()),
        ("sequence", F::integer(1, 1000).required()),
    ];
    let settings = vec![
        ("dpi", F::integer(150, 600).required()),
        ("duplex", F::boolean().required()),
        ("format", F::choice(&["png", "tiff"]).required()),
    ];
    let mut scan=T::new("scanner_start_scan","jobs.create","Physically scan loaded suitable media. Confirm media readiness first. Reuse requestID and identical settings for retries; a new UUID means another feed. Always enforces 600 DPI. Defaults to simplex PNG. Explicitly choose en or ko for prepared cards; English is the default, never automatic language detection. Poll job state and errors.",vec![
        ("requestID",F::uuid().required()),("deviceID",F::text(512)),("outputDirectory",F::text(2048)),("profileID",F::uuid()),("useDeviceSettings",F::boolean()),("prepareCards",F::boolean()),("cardLanguage",F::choice(&["en","ko"])),("dpi",F::integer(150,600)),("duplex",F::boolean()),("format",F::choice(&["png","tiff"]))]).writes(true);
    scan.defaults = json!({"dpi":600,"duplex":false,"format":"png"})
        .as_object()
        .unwrap()
        .clone();
    vec![
        T::new("scanner_prepare_card","images.prepareCard","Crop a saved card and orient readable text into an immutable PNG. Originals remain. Inspect warnings for uncertain orientation or clipping. Never feeds hardware. Reuse id for retries.",[image.clone(),vec![("id",F::uuid().required())]].concat()).writes(false),
        T::new("scanner_identify_card","images.identify","Start native OCR and catalogue matching on a saved front. Never scans hardware or uploads images. Reuse id for retries and poll identification. Language must match the batch; defaults to its physical card language. Suggestions need manual confirmation.",[image.clone(),vec![("id",F::uuid().required()),("language",F::choice(&["en","ko"]))]].concat()).writes(false).external(),
        T::new("scanner_get_card_identification","images.identification","Read identification progress, OCR evidence, candidates and manual review metadata.",image.clone()),
        T::new("scanner_review_card","images.review","Record the user's selected candidate, finish and condition. Never infer condition from OCR. Omit cardID to reject suggestions. identificationID prevents reviewing a replaced result.",[image.clone(),vec![("identificationID",F::uuid().required()),("cardID",F::text(100)),("finish",F::choice(&["unverified","normal","holo","reverseHolo","other"])),("condition",F::choice(&["unassessed","nearMint","lightlyPlayed","moderatelyPlayed","heavilyPlayed","damaged"]))]].concat()).writes(false),
        T::new("scanner_list_devices","devices","Discover connected scanners without feeding paper.",vec![]),
        T::new("scanner_get_device","device.get","Read readiness and configured defaults. Paper loaded state can be unknown. Never feeds paper.",device.clone()),
        T::new("scanner_get_capabilities","capabilities","Inspect driver capabilities and explicit unknowns. Requires idle hardware; never scans.",device.clone()),
        T::new("scanner_get_settings","settings","Read observed settings and saved defaults. Requires idle hardware.",device.clone()),
        T::new("scanner_set_settings","settings.set","Validate and save future defaults without feeding paper. Rejects busy hardware or unsupported values.",[settings.clone(),device.clone()].concat()).writes(false),
        T::new("scanner_list_profiles","profiles.list","List saved scan profiles.",vec![]),
        T::new("scanner_get_profile","profiles.get","Read a profile by UUID or unique name.",vec![("id",F::text(512).required())]),
        T::new("scanner_create_profile","profiles.create","Create a profile. Reuse the same ID and fields for retries. Conflicting names or IDs fail. This does not validate media feeding safety.",[settings,vec![("id",F::uuid().required()),("name",F::text(128).required())]].concat()).writes(false),
        T::new("scanner_update_profile","profiles.update","Update supplied fields only. Existing jobs retain their snapshots. Identify by UUID or unique name.",vec![("id",F::text(512).required()),("name",F::text(128)),("dpi",F::integer(150,600)),("duplex",F::boolean()),("format",F::choice(&["png","tiff"]))]).writes(false),
        scan,
        T::new("scanner_get_job","jobs.get","Poll state, saved images and errors. A successful query does not imply a successful scan.",id.clone()),
        T::new("scanner_list_jobs","jobs.list","Read persistent job history with bounded pagination.",page.clone()),
        T::new("scanner_cancel_job","jobs.cancel","Cancel a queued or active scan explicitly. Originals remain; active cancellation is asynchronous. Poll until terminal.",id.clone()).writes(true),
        T::new("scanner_list_batches","batches.list","List scan batches, one per job.",page),
        T::new("scanner_continue_batch","batches.continue","Physically scan another feed into a completed batch. Confirm media readiness first. Reuse requestID for retries. Preserves settings and numbering; failed or incomplete pairs cannot continue. Maximum 50 duplex cards per feed, 500 per batch.",vec![("id",F::uuid().required()),("requestID",F::uuid().required())]).writes(true),
        T::new("scanner_get_batch","batches.get","Read batch manifest, original dimensions, paths and SHA-256.",id.clone()),
        T::new("scanner_get_scan_metadata","jobs.get","Read complete scan record, settings, images, errors and manifest metadata.",id.clone()),
        T::new("scanner_get_batch_manifest","batches.get","Read the batch manifest from the service ledger.",id),
        T::new("scanner_get_image_metadata","images.get","Read original image metadata by job UUID and page sequence.",image.clone()),
        T::new("scanner_read_image","images.read","Read a bounded base64 chunk of an original. Follow nextOffset until eof; verify chunkSHA256 and the final image SHA-256. Accepts no filesystem path.",[image,vec![("offset",F::integer(0,512*1024*1024)),("length",F::integer(1,262144))]].concat()),
        T::new("scanner_get_service_status","service.status","Read queue and health without opening hardware.",vec![]),
        T::new("scanner_get_events","events","Read events, optionally waiting up to 20 seconds. Use nextCursor; on resetRequired refresh snapshots and start after=0.",vec![("after",F::integer(0,9_007_199_254_740_991)),("limit",F::integer(1,100)),("waitSeconds",F::integer(0,20))]),
        T::new("scanner_get_diagnostics","diagnostics","Inspect service and driver diagnostics without feeding. Hardware inspection can return busy; status and events remain available.",device),
    ]
}
