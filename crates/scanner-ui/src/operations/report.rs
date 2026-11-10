use rune::{
    El, col, text,
    tok::{self, type_},
};
use scanner_types::{
    device::{Evidence, Inspection},
    service::DeviceStatus,
};

pub(super) fn view(report: &DeviceStatus, t: tok::Theme) -> El {
    let mut pane = col().gap(8.);
    pane = pane.child(line(
        format!(
            "Driver: {} · {}",
            report.device.driver_path,
            report
                .device
                .driver_module_version
                .as_deref()
                .unwrap_or("version unknown")
        ),
        t,
    ));
    pane = pane.children(report.warnings.iter().map(|w| line(w.clone(), t)));
    let Some(inspection) = &report.inspection else {
        return pane;
    };
    pane = pane.child(line(
        format!(
            "Observed {} · session opened: {} · closed: {}",
            inspection.observed_at, inspection.session_opened, inspection.session_closed
        ),
        t,
    ));
    pane = pane.children(notes(inspection).into_iter().map(|n| line(n, t)));
    for unit in &inspection.units {
        let mut card = col()
            .gap(6.)
            .padding(12.)
            .radius(8.)
            .fill(t.card)
            .child(line(
                format!("{} · unit {}", unit.class_name, unit.unit_type),
                t,
            ));
        for (name, setting) in &unit.settings {
            let evidence = match setting.evidence {
                Evidence::HardwareTested => "hardware tested",
                Evidence::DriverExposedNotTested => "driver reported; not hardware tested",
                Evidence::SoftwareImplemented => "software implemented",
                Evidence::Unsupported => "unsupported",
                Evidence::Unknown => "unknown",
            };
            card = card.child(line(
                format!(
                    "{name}: {} · {evidence}{}",
                    setting.current,
                    if setting.read_only {
                        " · read only"
                    } else {
                        ""
                    }
                ),
                t,
            ));
            if let Some(allowed) = &setting.allowed {
                card = card.child(line(format!("Allowed: {allowed}"), t));
            }
            if let Some(default) = &setting.default_value {
                card = card.child(line(format!("Driver default: {default}"), t));
            }
        }
        for feature in &unit.vendor_features {
            card = card.child(line(format!("Vendor feature: {feature}"), t));
        }
        for template in &unit.templates {
            card = card.child(line(format!("Template: {template}"), t));
        }
        pane = pane.child(card);
    }
    pane
}
fn notes(i: &Inspection) -> Vec<String> {
    i.errors
        .iter()
        .map(ToString::to_string)
        .chain(i.warnings.iter().cloned())
        .chain(
            i.unknown_capabilities
                .iter()
                .map(|c| format!("Unknown capability: {c}")),
        )
        .chain(i.limitations.iter().cloned())
        .collect()
}
fn line(value: String, t: tok::Theme) -> El {
    text(value).font(type_::BODY).color(t.text_secondary).wrap()
}
