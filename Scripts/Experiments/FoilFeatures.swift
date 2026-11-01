// Offline experiment only. Reads prepared images and writes measurements; no
// scanner, catalogue, network, database or finish-review mutations.
import Foundation
import CoreGraphics
import ImageIO

struct Input: Codable {
    var id: String
    var path: String
    var label: String
    var printingIDs: [String]
}
struct Region: Codable {
    var meanBrightness: Double
    var meanChroma: Double
    var rowBandEnergy: Double
}
struct Measurement: Codable {
    var input: Input
    var width: Int
    var height: Int
    var leftBorder: Region
    var rightBorder: Region
    var artwork: Region
    var textBody: Region
    var borderBandCorrelation: Double
    var pairedBorderBandEnergy: Double
}
struct Raster {
    var pixels: [UInt8]
    var width: Int
    var height: Int
    func region(_ x0: Double, _ y0: Double, _ x1: Double, _ y1: Double) -> (Region, [Double]) {
        var rows: [Double] = [], brightness = 0.0, chroma = 0.0, count = 0
        for y in Int(y0 * Double(height))..<Int(y1 * Double(height)) {
            var row = 0.0, n = 0
            for x in Int(x0 * Double(width))..<Int(x1 * Double(width)) {
                let i = (y * width + x) * 4
                let r = Double(pixels[i]) / 255, g = Double(pixels[i+1]) / 255, b = Double(pixels[i+2]) / 255
                let value = 0.2126*r + 0.7152*g + 0.0722*b
                row += value; brightness += value; chroma += max(r,g,b)-min(r,g,b); count += 1; n += 1
            }
            rows.append(row / Double(max(1,n)))
        }
        // Remove slow illumination/printed-border changes, retaining fine bands.
        let residual = rows.indices.map { i in
            let lo = max(0,i-5), hi = min(rows.count-1,i+5)
            return rows[i] - rows[lo...hi].reduce(0,+) / Double(hi-lo+1)
        }
        let energy = sqrt(residual.reduce(0) { $0 + $1*$1 } / Double(max(1,residual.count)))
        return (Region(meanBrightness: brightness/Double(max(1,count)), meanChroma: chroma/Double(max(1,count)), rowBandEnergy: energy), residual)
    }
    func measure(_ input: Input) -> Measurement {
        let (left,a) = region(0.015,0.08,0.04,0.9), (right,b) = region(0.96,0.08,0.985,0.9)
        let (art,_) = region(0.12,0.12,0.88,0.46), (body,_) = region(0.12,0.58,0.88,0.86)
        let denominator = sqrt(a.reduce(0){$0+$1*$1} * b.reduce(0){$0+$1*$1})
        let correlation = denominator > 1e-12 ? zip(a,b).reduce(0){$0+$1.0*$1.1}/denominator : 0
        return Measurement(input: input, width: width, height: height, leftBorder: left, rightBorder: right,
            artwork: art, textBody: body, borderBandCorrelation: correlation,
            pairedBorderBandEnergy: min(left.rowBandEnergy,right.rowBandEnergy)*max(0,correlation))
    }
}
func load(_ path: String) throws -> Raster {
    let url = URL(fileURLWithPath: path)
    guard try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? Int.max <= 32*1024*1024,
          let source = CGImageSourceCreateWithURL(url as CFURL,nil), CGImageSourceGetCount(source)==1,
          let properties = CGImageSourceCopyPropertiesAtIndex(source,0,nil) as? [CFString:Any],
          let w = properties[kCGImagePropertyPixelWidth] as? Int, let h = properties[kCGImagePropertyPixelHeight] as? Int,
          w > 0, h > 0, w <= 20000, h <= 20000, w*h <= 40_000_000,
          let image = CGImageSourceCreateThumbnailAtIndex(source,0,[kCGImageSourceCreateThumbnailFromImageAlways:true,
              kCGImageSourceCreateThumbnailWithTransform:true,kCGImageSourceThumbnailMaxPixelSize:1260] as CFDictionary) else {
        throw NSError(domain:"FoilExperiment",code:1,userInfo:[NSLocalizedDescriptionKey:"Invalid or oversized prepared image"])
    }
    let width=image.width, height=image.height
    var pixels=[UInt8](repeating:0,count:width*height*4)
    guard let context=CGContext(data:&pixels,width:width,height:height,bitsPerComponent:8,bytesPerRow:width*4,
        space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGBitmapInfo.byteOrder32Big.rawValue|CGImageAlphaInfo.premultipliedLast.rawValue) else {
        throw NSError(domain:"FoilExperiment",code:2)
    }
    context.draw(image,in:CGRect(x:0,y:0,width:width,height:height))
    return Raster(pixels:pixels,width:width,height:height)
}
let args=CommandLine.arguments
if args.count==2 && args[1]=="--self-test" {
    let input=Input(id:"synthetic",path:"",label:"synthetic",printingIDs:[])
    var flat=Raster(pixels:[UInt8](repeating:128,count:100*140*4),width:100,height:140)
    assert(flat.measure(input).pairedBorderBandEnergy < 1e-12)
    for y in 0..<140 { for x in 0..<100 { for c in 0..<3 {
        flat.pixels[(y*100+x)*4+c] = y%4<2 ? 80 : 180
    } } }
    let bands=flat.measure(input)
    assert(bands.borderBandCorrelation > 0.99 && bands.pairedBorderBandEnergy > 0.1)
    print("Feature checks passed; no classification or accuracy claim.")
} else {
    guard args.count==3 else { fatalError("Usage: swift FoilFeatures.swift inputs.json output.json | --self-test") }
    let inputs=try JSONDecoder().decode([Input].self,from:Data(contentsOf:URL(fileURLWithPath:args[1])))
    guard !inputs.isEmpty && inputs.count<=200 else { fatalError("Expected 1–200 prepared images") }
    let results=try inputs.map { input in try autoreleasepool { try load(input.path).measure(input) } }
    let encoder=JSONEncoder();encoder.outputFormatting=[.prettyPrinted,.sortedKeys]
    try encoder.encode(results).write(to:URL(fileURLWithPath:args[2]),options:.atomic)
    print("Measured \(results.count) images; no finish values were changed.")
}
