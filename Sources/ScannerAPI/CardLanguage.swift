import Foundation

/// The physical cards' configured language, not the language of a translated catalogue view.
public enum CardLanguage: String, Codable, Sendable, CaseIterable {
    case english = "en"
    case korean = "ko"

    public var displayName: String { self == .english ? "English" : "Korean" }
    public var recognitionLanguages: [String] { self == .english ? ["en-US"] : ["ko-KR", "en-US"] }
}
