import CoreGraphics
import CoreText
import Foundation
import ImageIO
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

private func koreanTextImage() throws -> Data {
    let width = 600, height = 840
    let context = try #require(CGContext(data: nil, width: width, height: height,
        bitsPerComponent: 8, bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    context.setFillColor(CGColor(gray: 1, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let font = CTFontCreateWithName("AppleSDGothicNeo-Regular" as CFString, 28, nil)
    for (index, text) in ["피카츄 전기 포켓몬", "상대에게 데미지를 준다", "기본 에너지 카드가 필요하다",
                          "이번 차례에는 공격할 수 없다", "포켓몬을 벤치로 이동시킨다", "카드를 한 장 뽑는다"].enumerated() {
        let string = NSAttributedString(string: text, attributes: [
            NSAttributedString.Key(kCTFontAttributeName as String): font,
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): CGColor(gray: 0, alpha: 1)])
        context.textPosition = CGPoint(x: 40, y: height - 80 - index * 110)
        CTLineDraw(CTLineCreateWithAttributedString(string), context)
    }
    let data = NSMutableData(), image = try #require(context.makeImage())
    let destination = try #require(CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(destination, image, nil)
    #expect(CGImageDestinationFinalize(destination))
    return data as Data
}

@Test func koreanTextCanOrientPreparedFrontAtEveryQuarterTurn() throws {
    let image = try koreanTextImage()
    for turns in 0..<4 {
        let rotated = try ImageProcessor.render(image, request: ImageEditRequest(jobID: UUID(), sequence: 1, quarterTurns: turns))
        let orientation = try CardProcessor.uprightRotation(rotated.data, language: .korean)
        #expect(orientation.verified && orientation.basis == "text")
        #expect(orientation.turns == (4 - turns) % 4)
    }
}
