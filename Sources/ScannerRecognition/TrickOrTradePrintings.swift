import Foundation
import ScannerAPI

/// Bounded primary market-catalogue evidence, captured 2026-11-01. It does not
/// establish a unique release year, authenticity, or manufacturer certification.
/// Sources/provenance: Docs/stamped-editions.md. No physical test labels are used.
enum TrickOrTradePrintings {
    struct Entry {
        let id: String, name: String
        let number: Int, total: Int, year: Int
        let finishes: [CardFinish]
        let product: Int
    }
    static func matches(_ card: CardCandidate) -> [CardPrintingVariant] {
        guard let number = Int(card.localID), let total = card.officialSetCount else { return [] }
        return entries.filter { $0.id == card.id && $0.name == card.name && $0.number == number && $0.total == total }.flatMap { entry in
            entry.finishes.map { finish in
                CardPrintingVariant(id: String(entry.product) + "-" + finish.rawValue, source: "tcgplayer",
                    finish: finish, stamps: ["trick-or-trade"], years: [entry.year])
            }
        }
    }
    static let entries: [Entry] = [
        Entry(id: "swsh6-57", name: "Gengar", number: 57, total: 198, year: 2022, finishes: [.holo], product: 283766),
        Entry(id: "swsh9-056", name: "Mewtwo", number: 56, total: 172, year: 2022, finishes: [.holo], product: 283785),
        Entry(id: "swsh10-059", name: "Mismagius", number: 59, total: 189, year: 2022, finishes: [.holo], product: 283786),
        Entry(id: "swsh9-062", name: "Dusknoir", number: 62, total: 172, year: 2022, finishes: [.holo], product: 283787),
        Entry(id: "swsh3-105", name: "Darkrai", number: 105, total: 189, year: 2022, finishes: [.holo], product: 283788),
        Entry(id: "swsh2-33", name: "Chandelure", number: 33, total: 192, year: 2022, finishes: [.holo], product: 283790),
        Entry(id: "swsh2-15", name: "Trevenant", number: 15, total: 192, year: 2022, finishes: [.holo], product: 283793),
        Entry(id: "swsh7-77", name: "Gourgeist", number: 77, total: 203, year: 2022, finishes: [.holo], product: 283795),
        Entry(id: "swsh3-81", name: "Mimikyu", number: 81, total: 189, year: 2022, finishes: [.holo], product: 283798),
        Entry(id: "swsh6-73", name: "Hatterene", number: 73, total: 198, year: 2022, finishes: [.holo], product: 283801),
        Entry(id: "swsh7-49", name: "Pikachu", number: 49, total: 203, year: 2022, finishes: [.normal], product: 283805),
        Entry(id: "swsh5-69", name: "Cubone", number: 69, total: 163, year: 2022, finishes: [.normal], product: 283807),
        Entry(id: "swsh5-89", name: "Zubat", number: 89, total: 163, year: 2022, finishes: [.normal], product: 283810),
        Entry(id: "swsh6-55", name: "Gastly", number: 55, total: 198, year: 2022, finishes: [.normal], product: 283811),
        Entry(id: "swsh6-56", name: "Haunter", number: 56, total: 198, year: 2022, finishes: [.normal], product: 283812),
        Entry(id: "swsh3-102", name: "Spinarak", number: 102, total: 189, year: 2022, finishes: [.normal], product: 283815),
        Entry(id: "swsh3-103", name: "Ariados", number: 103, total: 189, year: 2022, finishes: [.normal], product: 283818),
        Entry(id: "swsh5-93", name: "Murkrow", number: 93, total: 163, year: 2022, finishes: [.normal], product: 283819),
        Entry(id: "swsh10-058", name: "Misdreavus", number: 58, total: 189, year: 2022, finishes: [.normal], product: 283820),
        Entry(id: "swsh9-060", name: "Duskull", number: 60, total: 172, year: 2022, finishes: [.normal], product: 283821),
        Entry(id: "swsh9-061", name: "Dusclops", number: 61, total: 172, year: 2022, finishes: [.normal], product: 283822),
        Entry(id: "swsh2-31", name: "Litwick", number: 31, total: 192, year: 2022, finishes: [.normal], product: 283824),
        Entry(id: "swsh2-32", name: "Lampent", number: 32, total: 192, year: 2022, finishes: [.normal], product: 283825),
        Entry(id: "swsh8-16", name: "Phantump", number: 16, total: 264, year: 2022, finishes: [.normal], product: 283826),
        Entry(id: "swsh7-76", name: "Pumpkaboo", number: 76, total: 203, year: 2022, finishes: [.normal], product: 283828),
        Entry(id: "swsh10-103", name: "Nickit", number: 103, total: 189, year: 2022, finishes: [.normal], product: 283831),
        Entry(id: "swsh3.5-18", name: "Hatenna", number: 18, total: 73, year: 2022, finishes: [.normal], product: 283833),
        Entry(id: "swsh6-72", name: "Hattrem", number: 72, total: 198, year: 2022, finishes: [.normal], product: 283836),
        Entry(id: "swsh3-82", name: "Sinistea", number: 82, total: 189, year: 2022, finishes: [.normal], product: 283838),
        Entry(id: "swsh3-83", name: "Polteageist", number: 83, total: 189, year: 2022, finishes: [.normal], product: 283839),
        Entry(id: "swsh11-016", name: "Phantump", number: 16, total: 196, year: 2023, finishes: [.normal], product: 515647),
        Entry(id: "swsh11-017", name: "Trevenant", number: 17, total: 196, year: 2023, finishes: [.holo], product: 515648),
        Entry(id: "swsh4-19", name: "Dhelmise", number: 19, total: 185, year: 2023, finishes: [.normal], product: 515649),
        Entry(id: "sv01-034", name: "Houndoom", number: 34, total: 198, year: 2023, finishes: [.holo], product: 515650),
        Entry(id: "swsh11-024", name: "Litwick", number: 24, total: 196, year: 2023, finishes: [.normal], product: 515651),
        Entry(id: "swsh11-025", name: "Lampent", number: 25, total: 196, year: 2023, finishes: [.normal], product: 515652),
        Entry(id: "swsh11-026", name: "Chandelure", number: 26, total: 196, year: 2023, finishes: [.holo], product: 515653),
        Entry(id: "sv02-062", name: "Pikachu", number: 62, total: 193, year: 2023, finishes: [.holo], product: 515654),
        Entry(id: "swsh4-95", name: "Lycanroc", number: 95, total: 185, year: 2023, finishes: [.normal], product: 515655),
        Entry(id: "swsh2-102", name: "Galarian Runerigus", number: 102, total: 192, year: 2023, finishes: [.normal], product: 515656),
        Entry(id: "swsh11-064", name: "Gastly", number: 64, total: 196, year: 2023, finishes: [.normal], product: 515659),
        Entry(id: "swsh11-065", name: "Haunter", number: 65, total: 196, year: 2023, finishes: [.normal], product: 515660),
        Entry(id: "swsh11-066", name: "Gengar", number: 66, total: 196, year: 2023, finishes: [.holo], product: 515661),
        Entry(id: "swsh4-69", name: "Duskull", number: 69, total: 185, year: 2023, finishes: [.normal], product: 515662),
        Entry(id: "swsh4-70", name: "Dusclops", number: 70, total: 185, year: 2023, finishes: [.normal], product: 515663),
        Entry(id: "swsh4-71", name: "Dusknoir", number: 71, total: 185, year: 2023, finishes: [.holo], product: 515664),
        Entry(id: "sv01-087", name: "Shuppet", number: 87, total: 198, year: 2023, finishes: [.normal], product: 515665),
        Entry(id: "swsh11-073", name: "Banette", number: 73, total: 196, year: 2023, finishes: [.normal], product: 515666),
        Entry(id: "sv02-088", name: "Mismagius", number: 88, total: 193, year: 2023, finishes: [.normal], product: 515668),
        Entry(id: "sv01-089", name: "Drifloon", number: 89, total: 198, year: 2023, finishes: [.normal], product: 515670),
        Entry(id: "sv01-090", name: "Drifblim", number: 90, total: 198, year: 2023, finishes: [.normal], product: 515671),
        Entry(id: "sv02-097", name: "Mimikyu", number: 97, total: 193, year: 2023, finishes: [.holo, .normal], product: 515672),
        Entry(id: "swsh7-80", name: "Marshadow", number: 80, total: 203, year: 2023, finishes: [.holo], product: 515673),
        Entry(id: "swsh11-081", name: "Spectrier", number: 81, total: 196, year: 2023, finishes: [.holo], product: 515674),
        Entry(id: "swsh1-89", name: "Sinistea", number: 89, total: 202, year: 2023, finishes: [.normal], product: 515675),
        Entry(id: "swsh1-90", name: "Polteageist", number: 90, total: 202, year: 2023, finishes: [.normal], product: 515676),
        Entry(id: "sv01-104", name: "Greavard", number: 104, total: 198, year: 2023, finishes: [.normal], product: 515677),
        Entry(id: "sv01-106", name: "Houndstone", number: 106, total: 198, year: 2023, finishes: [.holo], product: 515678),
        Entry(id: "swsh12-103", name: "Zubat", number: 103, total: 195, year: 2023, finishes: [.normal], product: 515679),
        Entry(id: "sv02-131", name: "Murkrow", number: 131, total: 193, year: 2023, finishes: [.normal], product: 515680),
        Entry(id: "sv02-012", name: "Sprigatito", number: 12, total: 193, year: 2024, finishes: [.normal], product: 568512),
        Entry(id: "sv02-050", name: "Quaxly", number: 50, total: 193, year: 2024, finishes: [.normal], product: 568513),
        Entry(id: "sv03-130", name: "Umbreon", number: 130, total: 197, year: 2024, finishes: [.normal], product: 568704),
        Entry(id: "sv03-131", name: "Houndour", number: 131, total: 197, year: 2024, finishes: [.normal], product: 568705),
        Entry(id: "sv03-133", name: "Houndoom", number: 133, total: 197, year: 2024, finishes: [.normal], product: 568748),
        Entry(id: "sv03-136", name: "Darkrai", number: 136, total: 197, year: 2024, finishes: [.holo], product: 568826),
        Entry(id: "sv04-023", name: "Fuecoco", number: 23, total: 182, year: 2024, finishes: [.normal], product: 568941),
        Entry(id: "sv04-077", name: "Pumpkaboo", number: 77, total: 182, year: 2024, finishes: [.normal], product: 568968),
        Entry(id: "sv04-078", name: "Gourgeist", number: 78, total: 182, year: 2024, finishes: [.normal], product: 569041),
        Entry(id: "sv04.5-018", name: "Pikachu", number: 18, total: 91, year: 2024, finishes: [.holo], product: 569132),
        Entry(id: "sv04.5-037", name: "Mimikyu", number: 37, total: 91, year: 2024, finishes: [.holo], product: 569227),
        Entry(id: "sv04.5-042", name: "Greavard", number: 42, total: 91, year: 2024, finishes: [.normal], product: 569228),
        Entry(id: "sv04.5-043", name: "Houndstone", number: 43, total: 91, year: 2024, finishes: [.normal], product: 569323),
        Entry(id: "sv04.5-057", name: "Gengar", number: 57, total: 91, year: 2024, finishes: [.holo], product: 570271),
        Entry(id: "sv05-077", name: "Scream Tail", number: 77, total: 162, year: 2024, finishes: [.normal], product: 570272),
        Entry(id: "sv05-078", name: "Flutter Mane", number: 78, total: 162, year: 2024, finishes: [.holo], product: 570273),
        Entry(id: "sv05-102", name: "Gastly", number: 102, total: 162, year: 2024, finishes: [.normal], product: 570320),
        Entry(id: "sv05-103", name: "Haunter", number: 103, total: 162, year: 2024, finishes: [.normal], product: 570361),
        Entry(id: "sv05-139", name: "Iron Jugulis", number: 139, total: 162, year: 2024, finishes: [.normal], product: 570362),
        Entry(id: "sv06-012", name: "Phantump", number: 12, total: 167, year: 2024, finishes: [.normal], product: 570363),
        Entry(id: "sv06-013", name: "Trevenant", number: 13, total: 167, year: 2024, finishes: [.normal], product: 570364),
        Entry(id: "sv06-021", name: "Poltchageist", number: 21, total: 167, year: 2024, finishes: [.normal], product: 570365),
        Entry(id: "sv06-022", name: "Sinistcha", number: 22, total: 167, year: 2024, finishes: [.holo], product: 570462),
        Entry(id: "sv06-024", name: "Teal Mask Ogerpon", number: 24, total: 167, year: 2024, finishes: [.holo], product: 570463),
        Entry(id: "sv06-036", name: "Litwick", number: 36, total: 167, year: 2024, finishes: [.normal], product: 570563),
        Entry(id: "sv06-037", name: "Lampent", number: 37, total: 167, year: 2024, finishes: [.normal], product: 570564),
        Entry(id: "sv06-038", name: "Chandelure", number: 38, total: 167, year: 2024, finishes: [.normal], product: 570565),
        Entry(id: "sv06-095", name: "Munkidori", number: 95, total: 167, year: 2024, finishes: [.holo], product: 570567),
        Entry(id: "sv06-096", name: "Fezandipiti", number: 96, total: 167, year: 2024, finishes: [.holo], product: 570568),
        Entry(id: "sv06-111", name: "Okidogi", number: 111, total: 167, year: 2024, finishes: [.holo], product: 570569),
    ]
}
