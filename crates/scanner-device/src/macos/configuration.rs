use objc2::{rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSNumber, NSPoint, NSRect, NSString, NSURL};
use objc2_image_capture_core::{
    ICScannerBitDepth, ICScannerDevice, ICScannerDocumentType, ICScannerFeatureEnumeration,
    ICScannerFunctionalUnit, ICScannerFunctionalUnitDocumentFeeder, ICScannerMeasurementUnit,
    ICScannerPixelDataType, ICScannerTransferMode,
};
use scanner_types::{
    Extensions,
    error::{Result, ScannerError},
    scan::{OriginalFormat, ScanBackground, ScanRequest},
};
use serde_json::json;
use std::path::Path;
#[derive(Default)]
pub(super) struct Configuration {
    restore: Vec<Box<dyn FnOnce()>>,
    values: Vec<Retained<NSNumber>>,
}
impl Configuration {
    pub fn restore(&mut self) {
        for restore in self.restore.drain(..).rev() {
            restore();
        }
        self.values.clear();
    }
    pub fn apply(
        &mut self,
        scanner: &Retained<ICScannerDevice>,
        request: &ScanRequest,
        incoming: &Path,
    ) -> Result<Extensions> {
        request.validate()?;
        // SAFETY: only main-thread operations call this module; every mutated
        // object and previous value is retained until explicit post-scan restore.
        unsafe {
            let feeder = scanner
                .selectedFunctionalUnit()
                .downcast::<ICScannerFunctionalUnitDocumentFeeder>()
                .map_err(|_| unsupported("Basic scanning requires an ADF."))?;
            validate_feeder(&feeder, request)?;
            self.save_feeder(&feeder);
            self.feature("blank_page_skip", 0, &feeder)?;
            let background = request.background.unwrap_or(ScanBackground::Black);
            let cropping = if request.overscan == Some(true) {
                4
            } else if background == ScanBackground::White {
                1
            } else {
                0
            };
            self.feature("cropping", cropping, &feeder)?;
            feeder.setMeasurementUnit(ICScannerMeasurementUnit(0));
            feeder.setDocumentType(ICScannerDocumentType(1));
            feeder.setPixelDataType(ICScannerPixelDataType::RGB);
            feeder.setBitDepth(ICScannerBitDepth(8));
            feeder.setResolution(request.dpi as usize);
            feeder.setDuplexScanningEnabled(request.duplex);
            let size = feeder.documentSize();
            if !size.width.is_finite()
                || !size.height.is_finite()
                || !(8.0..9.0).contains(&size.width)
                || !(11.0..12.0).contains(&size.height)
            {
                return Err(rejected(
                    "Driver did not provide usable A4 dimensions in inches.",
                ));
            }
            feeder.setScanArea(NSRect::new(NSPoint::new(0., 0.), size));
            let actual = feeder.scanArea();
            if feeder.measurementUnit().0 != 0
                || feeder.documentType().0 != 1
                || feeder.pixelDataType() != ICScannerPixelDataType::RGB
                || feeder.bitDepth().0 != 8
                || feeder.resolution() != request.dpi as usize
                || feeder.duplexScanningEnabled() != request.duplex
                || (actual.size.width - size.width).abs() >= 0.01
                || (actual.size.height - size.height).abs() >= 0.01
            {
                return Err(rejected(
                    "Driver rejected explicit scan settings; scan not started.",
                ));
            }
            self.configure_transfer(scanner, request, incoming)?;
            let value = json!({"paper":"A4","dpi":request.dpi,"duplex":request.duplex,"pixelMode":"RGB","bitsPerChannel":8,
                "format":request.format,"scanAreaInches":[0,0,size.width,size.height],"blankPageSkip":0,"cropping":cropping,
                "background":background,"overscan":request.overscan==Some(true),"multifeed":"unchanged"});
            serde_json::from_value(value).map_err(|e| rejected(&e.to_string()))
        }
    }
    fn configure_transfer(
        &mut self,
        scanner: &Retained<ICScannerDevice>,
        request: &ScanRequest,
        incoming: &Path,
    ) -> Result<()> {
        // SAFETY: retained scanner, main-thread-only configuration and restoration.
        unsafe {
            self.save_transfer(scanner);
            let path = incoming
                .to_str()
                .ok_or_else(|| rejected("Driver output path must be UTF-8."))?;
            scanner.setTransferMode(ICScannerTransferMode::FileBased);
            scanner.setDownloadsDirectory(&NSURL::fileURLWithPath_isDirectory(
                &NSString::from_str(path),
                true,
            ));
            scanner.setDocumentName(&NSString::from_str(&format!(
                "capture-{}",
                request.request_id
            )));
            scanner.setDocumentUTI(&NSString::from_str(match request.format {
                OriginalFormat::Png => "public.png",
                OriginalFormat::Tiff => "public.tiff",
            }));
        }
        Ok(())
    }
    fn save_feeder(&mut self, feeder: &Retained<ICScannerFunctionalUnitDocumentFeeder>) {
        unsafe {
            let previous = (
                feeder.measurementUnit(),
                feeder.documentType(),
                feeder.pixelDataType(),
                feeder.bitDepth(),
                feeder.resolution(),
                feeder.duplexScanningEnabled(),
                feeder.scanArea(),
            );
            let feeder = feeder.clone();
            self.restore.push(Box::new(move || {
                feeder.setMeasurementUnit(previous.0);
                feeder.setDocumentType(previous.1);
                feeder.setPixelDataType(previous.2);
                feeder.setBitDepth(previous.3);
                feeder.setResolution(previous.4);
                feeder.setDuplexScanningEnabled(previous.5);
                feeder.setScanArea(previous.6);
            }));
        }
    }
    fn save_transfer(&mut self, scanner: &Retained<ICScannerDevice>) {
        unsafe {
            let previous = (
                scanner.transferMode(),
                scanner.downloadsDirectory(),
                scanner.documentName(),
                scanner.documentUTI(),
            );
            let scanner = scanner.clone();
            self.restore.push(Box::new(move || {
                scanner.setTransferMode(previous.0);
                scanner.setDownloadsDirectory(&previous.1);
                scanner.setDocumentName(&previous.2);
                scanner.setDocumentUTI(&previous.3);
            }));
        }
    }
    fn feature(&mut self, name: &str, value: i32, unit: &ICScannerFunctionalUnit) -> Result<()> {
        unsafe {
            let feature = unit
                .vendorFeatures()
                .and_then(|features| {
                    features
                        .iter()
                        .find(|f| f.internalName().is_some_and(|n| n.to_string() == name))
                })
                .and_then(|f| f.downcast::<ICScannerFeatureEnumeration>().ok())
                .ok_or_else(|| {
                    unsupported(&format!(
                        "Driver does not expose required {name}; scan not started."
                    ))
                })?;
            let chosen = feature
                .values()
                .iter()
                .find(|n| n.intValue() == value)
                .ok_or_else(|| {
                    unsupported(&format!("Driver does not expose required {name}={value}."))
                })?;
            let previous: Retained<AnyObject> = feature.currentValue();
            let restoring = feature.clone();
            self.restore
                .push(Box::new(move || restoring.setCurrentValue(&previous)));
            self.values.push(chosen.clone());
            feature.setCurrentValue(&chosen);
            if feature
                .currentValue()
                .downcast::<NSNumber>()
                .ok()
                .is_none_or(|n| n.intValue() != value)
            {
                return Err(rejected(&format!("Driver rejected {name}={value}.")));
            }
            Ok(())
        }
    }
}
fn unsupported(message: &str) -> ScannerError {
    ScannerError::new("unsupportedSetting", message)
}
fn rejected(message: &str) -> ScannerError {
    ScannerError::new("invalidSettings", message)
}

fn validate_feeder(
    feeder: &ICScannerFunctionalUnitDocumentFeeder,
    request: &ScanRequest,
) -> Result<()> {
    // SAFETY: selected retained feeder on the main thread; read-only capability checks.
    unsafe {
        if !feeder
            .supportedResolutions()
            .containsIndex(request.dpi as usize)
            || !feeder.supportedBitDepths().containsIndex(8)
            || !feeder.supportedMeasurementUnits().containsIndex(0)
            || !feeder.supportedDocumentTypes().containsIndex(1)
        {
            return Err(unsupported(
                "Driver must expose the requested DPI, 8-bit depth, inches and A4.",
            ));
        }
        if request.duplex && !feeder.supportsDuplexScanning() {
            return Err(unsupported("Duplex is not supported by this driver."));
        }
        if !feeder.documentLoaded() {
            return Err(ScannerError::new(
                "feederEmpty",
                "Load the intended media before starting a scan.",
            ));
        }
        if feeder.scanInProgress() {
            return Err(ScannerError::new("busy", "Scanner is already scanning."));
        }
    }
    Ok(())
}
