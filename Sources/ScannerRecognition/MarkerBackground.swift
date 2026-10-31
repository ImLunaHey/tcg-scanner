import Foundation

/// Measures broad, uninterrupted background patches rather than counting glyph
/// pixels as background. Thin outlines and narrow letter stems do not qualify.
struct MarkerBackground {
    var darkArea: Double
    var lightArea: Double

    var polarity: String? {
        if darkArea >= 0.05 && darkArea > lightArea * 3 { return "dark" }
        if lightArea >= 0.05 && lightArea > darkArea * 3 { return "light" }
        return nil
    }

    static func measure(_ values: [Double], width: Int, height: Int) -> Self {
        guard width >= 12, height >= 6, values.count == width * height else {
            return Self(darkArea: 0, lightArea: 0)
        }
        let inset = max(1, width / 20)
        let innerWidth = width - inset * 2
        let minimumWidth = max(4, Int(ceil(Double(innerWidth) * 0.18)))
        let minimumHeight = max(3, Int(ceil(Double(height) * 0.12)))
        func largestRectangle(dark: Bool) -> Double {
            var heights = [Int](repeating: 0, count: innerWidth + 1)
            var largest = 0
            for y in 0..<height {
                for x in 0..<innerWidth {
                    let value = values[y * width + x + inset]
                    let matches = dark ? value < 0.30 : value > 0.70
                    heights[x] = matches ? heights[x] + 1 : 0
                }
                // Monotonic histogram stack: O(width * height), including the
                // sentinel column that flushes rectangles ending at the right edge.
                var stack: [(start: Int, height: Int)] = []
                for x in 0...innerWidth {
                    var start = x
                    while let last = stack.last, last.height > heights[x] {
                        stack.removeLast()
                        let span = x - last.start
                        if span >= minimumWidth && last.height >= minimumHeight {
                            largest = max(largest, span * last.height)
                        }
                        start = last.start
                    }
                    if stack.last?.height != heights[x] {
                        stack.append((start, heights[x]))
                    }
                }
            }
            return Double(largest) / Double(innerWidth * height)
        }
        return Self(darkArea: largestRectangle(dark: true), lightArea: largestRectangle(dark: false))
    }
}
