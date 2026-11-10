use objc2::runtime::AnyObject;
use objc2_foundation::{NSArray, NSDictionary, NSIndexSet, NSNotFound, NSNumber, NSString};
use objc2_image_capture_core::{
    ICScannerFeature, ICScannerFeatureBoolean, ICScannerFeatureEnumeration, ICScannerFeatureRange,
    ICScannerFeatureTemplate, ICScannerFunctionalUnit, ICScannerFunctionalUnitDocumentFeeder,
};
use scanner_types::device::{Evidence, Setting, UnitSnapshot};
use serde_json::{Value, json};

pub(super) fn snapshot(unit: &ICScannerFunctionalUnit) -> UnitSnapshot {
    // SAFETY: all snapshot reads occur on the main thread while this unit is selected.
    let mut out = unsafe {
        UnitSnapshot {
            unit_type: unit.r#type().0 as u32,
            class_name: unit.class().name().to_string_lossy().into(),
            vendor_features: unit
                .vendorFeatures()
                .map(|f| f.iter().take(256).map(|f| feature(&f)).collect())
                .unwrap_or_default(),
            templates: unit
                .templates()
                .iter()
                .take(128)
                .map(|f| feature(&f))
                .collect(),
            ..UnitSnapshot::default()
        }
    };
    common(unit, &mut out);
    geometry(unit, &mut out);
    status(unit, &mut out);
    if let Some(feeder) = unit.downcast_ref::<ICScannerFunctionalUnitDocumentFeeder>() {
        adf(feeder, &mut out);
    }
    out
}
fn insert(
    out: &mut UnitSnapshot,
    name: &str,
    current: Value,
    allowed: Option<Value>,
    read_only: bool,
) {
    out.settings.insert(
        name.into(),
        Setting {
            current,
            allowed,
            default_value: None,
            read_only,
            evidence: Evidence::DriverExposedNotTested,
        },
    );
}
fn common(u: &ICScannerFunctionalUnit, out: &mut UnitSnapshot) {
    unsafe {
        insert(
            out,
            "resolutionDPI",
            json!(u.resolution()),
            Some(indices(&u.supportedResolutions())),
            false,
        );
        insert(
            out,
            "preferredResolutionsDPI",
            indices(&u.preferredResolutions()),
            None,
            true,
        );
        insert(
            out,
            "nativeResolutionDPI",
            json!([u.nativeXResolution(), u.nativeYResolution()]),
            None,
            true,
        );
        insert(
            out,
            "pixelDataType",
            json!(u.pixelDataType().0),
            None,
            false,
        );
        insert(
            out,
            "bitDepth",
            json!(u.bitDepth().0),
            Some(indices(&u.supportedBitDepths())),
            false,
        );
        insert(
            out,
            "measurementUnit",
            json!(u.measurementUnit().0),
            Some(indices(&u.supportedMeasurementUnits())),
            false,
        );
        insert(
            out,
            "scaleFactorPercent",
            json!(u.scaleFactor()),
            Some(indices(&u.supportedScaleFactors())),
            false,
        );
        insert(
            out,
            "preferredScaleFactors",
            indices(&u.preferredScaleFactors()),
            None,
            true,
        );
    }
}
fn geometry(u: &ICScannerFunctionalUnit, out: &mut UnitSnapshot) {
    unsafe {
        let size = u.physicalSize();
        let rect = u.scanArea();
        insert(
            out,
            "physicalSize",
            json!([size.width, size.height]),
            None,
            true,
        );
        insert(
            out,
            "scanAreaXYWH",
            json!([
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height
            ]),
            None,
            false,
        );
        insert(
            out,
            "scanAreaOrientation",
            json!(u.scanAreaOrientation().0),
            None,
            false,
        );
        insert(
            out,
            "acceptsThreshold",
            json!(u.acceptsThresholdForBlackAndWhiteScanning()),
            None,
            true,
        );
        insert(
            out,
            "usesThreshold",
            json!(u.usesThresholdForBlackAndWhiteScanning()),
            None,
            false,
        );
        let threshold = Setting {
            current: json!(u.thresholdForBlackAndWhiteScanning()),
            allowed: None,
            default_value: Some(json!(u.defaultThresholdForBlackAndWhiteScanning())),
            read_only: false,
            evidence: Evidence::DriverExposedNotTested,
        };
        out.settings.insert("threshold".into(), threshold);
    }
}
fn status(u: &ICScannerFunctionalUnit, out: &mut UnitSnapshot) {
    unsafe {
        for (name, value, read_only) in [
            ("state", json!(u.state().0), true),
            ("scanInProgress", json!(u.scanInProgress()), true),
            (
                "scanProgressPercent",
                json!(u.scanProgressPercentDone()),
                true,
            ),
            (
                "canPerformOverviewScan",
                json!(u.canPerformOverviewScan()),
                true,
            ),
            (
                "overviewScanInProgress",
                json!(u.overviewScanInProgress()),
                true,
            ),
            ("overviewResolution", json!(u.overviewResolution()), false),
        ] {
            insert(out, name, value, None, read_only);
        }
    }
}
fn adf(f: &ICScannerFunctionalUnitDocumentFeeder, out: &mut UnitSnapshot) {
    unsafe {
        insert(
            out,
            "documentType",
            json!(f.documentType().0),
            Some(indices(&f.supportedDocumentTypes())),
            false,
        );
        let size = f.documentSize();
        insert(
            out,
            "documentSize",
            json!([size.width, size.height]),
            None,
            true,
        );
        for (name, value, read_only) in [
            (
                "supportsDuplexScanning",
                json!(f.supportsDuplexScanning()),
                true,
            ),
            (
                "duplexScanningEnabled",
                json!(f.duplexScanningEnabled()),
                false,
            ),
            ("documentLoaded", json!(f.documentLoaded()), true),
            ("oddPageOrientation", json!(f.oddPageOrientation().0), false),
            (
                "evenPageOrientation",
                json!(f.evenPageOrientation().0),
                false,
            ),
            (
                "reverseFeederPageOrder",
                json!(f.reverseFeederPageOrder()),
                true,
            ),
        ] {
            insert(out, name, value, None, read_only);
        }
    }
}
fn indices(set: &NSIndexSet) -> Value {
    let mut values = vec![];
    let mut value = set.firstIndex();
    while value != NSNotFound as usize && values.len() < 4096 {
        values.push(value);
        value = set.indexGreaterThanIndex(value);
    }
    json!(values)
}
fn feature(f: &ICScannerFeature) -> Value {
    unsafe {
        let (fields, read_only) = if let Some(e) = f.downcast_ref::<ICScannerFeatureEnumeration>() {
            (
                json!({"current":value(&e.currentValue(),0),"default":value(&e.defaultValue(),0),
                "values":e.values().iter().take(256).map(|n|n.doubleValue()).collect::<Vec<_>>(),
                "labels":e.menuItemLabels().iter().take(256).map(|s|s.to_string()).collect::<Vec<_>>(),
                "tooltips":e.menuItemLabelsTooltips().iter().take(256).map(|s|s.to_string()).collect::<Vec<_>>()}),
                Some(false),
            )
        } else if let Some(r) = f.downcast_ref::<ICScannerFeatureRange>() {
            (
                json!({"current":r.currentValue(),"default":r.defaultValue(),"minimum":r.minValue(),"maximum":r.maxValue(),"step":r.stepSize()}),
                Some(false),
            )
        } else if let Some(b) = f.downcast_ref::<ICScannerFeatureBoolean>() {
            (json!({"current":b.value()}), Some(false))
        } else if let Some(t) = f.downcast_ref::<ICScannerFeatureTemplate>() {
            (json!({"targets":value(&t.targets(),0)}), Some(true))
        } else {
            (json!({}), None)
        };
        json!({"internalName":f.internalName().map(|s|s.to_string()),"label":f.humanReadableName().map(|s|s.to_string()),"tooltip":f.tooltip().map(|s|s.to_string()),
            "type":f.r#type().0,"fields":fields,"readOnly":read_only,"evidence":"driverExposedNotTested"})
    }
}
fn value(object: &AnyObject, depth: usize) -> Value {
    if depth > 4 {
        return Value::Null;
    }
    if let Some(number) = object.downcast_ref::<NSNumber>() {
        return json!(number.doubleValue());
    }
    if let Some(text) = object.downcast_ref::<NSString>() {
        return json!(text.to_string());
    }
    if let Some(array) = object.downcast_ref::<NSArray>() {
        return Value::Array(
            array
                .iter()
                .take(256)
                .map(|v| value(&v, depth + 1))
                .collect(),
        );
    }
    if let Some(dictionary) = object.downcast_ref::<NSDictionary>() {
        return Value::Object(
            dictionary
                .keys()
                .take(256)
                .filter_map(|key| {
                    let v = dictionary.objectForKey(&key)?;
                    key.downcast_ref::<NSString>()
                        .map(|key| (key.to_string(), value(&v, depth + 1)))
                })
                .collect(),
        );
    }
    json!({"unrecognizedType":object.class().name().to_string_lossy()})
}
