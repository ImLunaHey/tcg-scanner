import ScannerAPI

public enum InspectionWarnings {
    public static func evaluate(_ units: [UnitSnapshot]) -> [String] {
        var warnings: [String] = []
        for unit in units {
            if case .array(let size) = unit.settings["physicalSize"]?.current,
               size.contains(.number(0)) {
                warnings.append("Unit \(unit.type): driver returned zero physical dimensions; do not treat these as usable scan bounds.")
            }
            for feature in unit.vendorFeatures {
                if case .array(let values) = feature.fields["values"],
                   case .array(let labels) = feature.fields["labels"], values.count != labels.count {
                    warnings.append("\(feature.internalName ?? "Unnamed feature"): values and labels have different counts; unlabelled values are unknown.")
                }
                if feature.internalName == "blank_page_skip", let current = feature.fields["current"], current != .number(0) {
                    warnings.append("Blank-page skipping is enabled; it may discard images and disrupt duplex pairing. No setting was changed.")
                }
            }
        }
        return warnings
    }
}
