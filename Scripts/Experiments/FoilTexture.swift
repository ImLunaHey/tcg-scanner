// Offline feature exploration. No scanner, network, metadata or finish mutations.
// swiftc -O FoilTexture.swift -o /tmp/foil-texture
// /tmp/foil-texture inputs.json measurements.json [--variants]
import Foundation
import CoreGraphics
import ImageIO

struct Input: Codable {
    let id: String
    let path: String
    let label: String
    let printingIDs: [String]
}
struct Measurement: Encodable {
    let input: Input
    let variant: String
    let width: Int
    let height: Int
    let features: [String: Double]
}
struct Patch {
    let rowEnergy: Double
    let colourRowEnergy: Double
    let meanChroma: Double
    let colourSpread: Double
    let brightnessSpread: Double
}
func rms(_ values: [Double]) -> Double {
    sqrt(values.reduce(0) { $0 + $1 * $1 } / Double(max(1, values.count)))
}
func residual(_ values: [Double], radius: Int) -> [Double] {
    values.indices.map { i in
        let lo = max(0, i-radius), hi = min(values.count-1, i+radius)
        return values[i] - values[lo...hi].reduce(0,+) / Double(hi-lo+1)
    }
}
func spread(_ values: [Double]) -> Double {
    let mean = values.reduce(0,+) / Double(max(1,values.count))
    return rms(values.map { $0 - mean })
}
func median(_ values: [Double]) -> Double {
    let v = values.sorted()
    return (v[(v.count-1)/2] + v[v.count/2]) / 2
}
struct Raster {
    let pixels: [UInt8]
    let width: Int
    let height: Int
    func patch(_ x0: Double, _ y0: Double, _ x1: Double, _ y1: Double) -> Patch {
        var lum: [Double] = [], rg: [Double] = [], bg: [Double] = [], chroma = 0.0
        var rowL: [Double] = [], rowRG: [Double] = [], rowBG: [Double] = []
        for y in Int(y0*Double(height))..<Int(y1*Double(height)) {
            var l = 0.0, a = 0.0, b = 0.0, count = 0
            for x in Int(x0*Double(width))..<Int(x1*Double(width)) {
                let i = (y*width+x)*4
                let r = Double(pixels[i])/255, g = Double(pixels[i+1])/255, blue = Double(pixels[i+2])/255
                let v = 0.2126*r + 0.7152*g + 0.0722*blue
                lum.append(v); rg.append(r-g); bg.append(blue-g)
                chroma += max(r,g,blue)-min(r,g,blue)
                l += v; a += r-g; b += blue-g; count += 1
            }
            rowL.append(l/Double(max(1,count))); rowRG.append(a/Double(max(1,count))); rowBG.append(b/Double(max(1,count)))
        }
        // Keep the smoothing span approximately constant in card-relative units.
        let radius = max(1,Int((5*Double(height)/1260).rounded()))
        return Patch(rowEnergy:rms(residual(rowL,radius:radius)),
            colourRowEnergy:hypot(rms(residual(rowRG,radius:radius)),rms(residual(rowBG,radius:radius))),
            meanChroma:chroma/Double(max(1,lum.count)), colourSpread:hypot(spread(rg),spread(bg)), brightnessSpread:spread(lum))
    }
    func measure(_ input: Input, variant: String) -> Measurement {
        // The inset variant tests narrower strips away from the crop edge/artwork.
        let inner = variant == "inset"
        let x0 = inner ? 0.02 : 0.015, x1 = inner ? 0.035 : 0.04
        let y0 = 0.12, y1 = 0.86
        let left = patch(x0,y0,x1,y1), right = patch(1-x1,y0,1-x0,y1)
        let art = patch(0.12,0.12,0.88,0.46), body = patch(0.12,0.58,0.88,0.86)
        // Median over eight sections reduces sensitivity to one mark or symbol.
        var tileL:[Patch] = [], tileR:[Patch] = []
        for tile in 0..<8 {
            let a = y0+(y1-y0)*Double(tile)/8, b = y0+(y1-y0)*Double(tile+1)/8
            tileL.append(patch(x0,a,x1,b)); tileR.append(patch(1-x1,a,1-x0,b))
        }
        let features: [String:Double] = [
            "borderMinimumBand":min(left.rowEnergy,right.rowEnergy),
            "borderMaximumBand":max(left.rowEnergy,right.rowEnergy),
            "borderMedianTileBand":min(median(tileL.map(\.rowEnergy)),median(tileR.map(\.rowEnergy))),
            "borderColourBand":min(left.colourRowEnergy,right.colourRowEnergy),
            "borderMedianTileColourBand":min(median(tileL.map(\.colourRowEnergy)),median(tileR.map(\.colourRowEnergy))),
            "borderChroma":min(left.meanChroma,right.meanChroma),
            "borderColourSpread":min(left.colourSpread,right.colourSpread),
            "borderBrightnessSpread":min(left.brightnessSpread,right.brightnessSpread),
            "artworkBand":art.rowEnergy,
            "artworkColourBand":art.colourRowEnergy,
            "bodyBand":body.rowEnergy,
            "bodyColourBand":body.colourRowEnergy
        ]
        return Measurement(input:input,variant:variant,width:width,height:height,features:features)
    }
}
func load(_ path: String, size: Int) throws -> Raster {
    let url = URL(fileURLWithPath:path)
    guard try url.resourceValues(forKeys:[.fileSizeKey]).fileSize ?? Int.max <= 32*1024*1024,
        let source = CGImageSourceCreateWithURL(url as CFURL,nil), CGImageSourceGetCount(source)==1,
        let props = CGImageSourceCopyPropertiesAtIndex(source,0,nil) as? [CFString:Any],
        let w = props[kCGImagePropertyPixelWidth] as? Int, let h = props[kCGImagePropertyPixelHeight] as? Int,
        w >= 100, h >= 100, w <= 20_000, h <= 20_000, w*h <= 40_000_000,
        let image = CGImageSourceCreateThumbnailAtIndex(source,0,[kCGImageSourceCreateThumbnailFromImageAlways:true,
            kCGImageSourceCreateThumbnailWithTransform:true,kCGImageSourceThumbnailMaxPixelSize:size] as CFDictionary)
    else { throw NSError(domain:"FoilTexture",code:1,userInfo:[NSLocalizedDescriptionKey:"Invalid prepared image"])}
    let width = image.width, height = image.height
    var pixels = [UInt8](repeating:0,count:width*height*4)
    // Pointer remains valid for drawing, rather than escaping an inout conversion.
    try pixels.withUnsafeMutableBytes { bytes in
        guard let ctx = CGContext(data:bytes.baseAddress,width:width,height:height,bitsPerComponent:8,bytesPerRow:width*4,
            space:CGColorSpace(name:CGColorSpace.sRGB)!,bitmapInfo:CGBitmapInfo.byteOrder32Big.rawValue|CGImageAlphaInfo.premultipliedLast.rawValue)
        else { throw NSError(domain:"FoilTexture",code:2) }
        ctx.draw(image,in:CGRect(x:0,y:0,width:width,height:height))
    }
    return Raster(pixels:pixels,width:width,height:height)
}
func selfTest() {
    let input = Input(id:"synthetic",path:"",label:"synthetic",printingIDs:[])
    func raster(_ pixel: (Int,Int)->[UInt8]) -> Raster {
        var data:[UInt8] = []
        for y in 0..<280 { for x in 0..<200 { data += pixel(x,y)+[255] } }
        return Raster(pixels:data,width:200,height:280)
    }
    let flat = raster { _,_ in [128,128,128] }.measure(input,variant:"baseline").features
    precondition(flat.values.allSatisfy { $0 < 1e-12 })
    // Opposite-phase borders must still register; the original correlation cue fails here.
    let bands = raster { x,y in let bright = (y%4<2) != (x>100); return [UInt8](repeating:bright ? 180:80,count:3) }
        .measure(input,variant:"baseline").features
    precondition(bands["borderMinimumBand"]! > 0.05 && bands["borderColourBand"]! < 1e-12)
    let colours = raster { _,y in y%4<2 ? [180,100,80]:[80,100,180] }.measure(input,variant:"baseline").features
    precondition(colours["borderColourBand"]! > 0.1)
    // One anomalous section should not dominate median-of-sections measurements.
    let spot = raster { _,y in [UInt8](repeating:y>50 && y<58 ? 220:128,count:3) }.measure(input,variant:"baseline").features
    precondition(spot["borderMinimumBand"]! > 0 && spot["borderMedianTileBand"]! < 1e-12)
    print("Texture checks passed: flat, opposite-phase bands, chromatic bands and isolated stripe.")
}
let args = CommandLine.arguments
if args.count == 2 && args[1] == "--self-test" { selfTest() }
else {
    guard args.count == 3 || (args.count == 4 && args[3] == "--variants") else {
        fatalError("Usage: FoilTexture inputs.json output.json [--variants] | --self-test")
    }
    let inputURL = URL(fileURLWithPath:args[1]), outputURL = URL(fileURLWithPath:args[2])
    guard outputURL.pathExtension == "json", !FileManager.default.fileExists(atPath:outputURL.path),
        try inputURL.resourceValues(forKeys:[.fileSizeKey]).fileSize ?? Int.max <= 1024*1024 else {
        fatalError("Expected small manifest and new JSON output path")
    }
    let inputs = try JSONDecoder().decode([Input].self,from:Data(contentsOf:inputURL))
    guard !inputs.isEmpty, inputs.count <= 200, Set(inputs.map(\.id)).count == inputs.count else { fatalError("Expected 1–200 unique inputs") }
    var results:[Measurement] = []
    for input in inputs {
        try autoreleasepool {
            let raster = try load(input.path,size:1260)
            results.append(raster.measure(input,variant:"baseline"))
            if args.count == 4 {
                results.append(raster.measure(input,variant:"inset"))
                results.append(try load(input.path,size:630).measure(input,variant:"630"))
            }
        }
    }
    let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted,.sortedKeys]
    try encoder.encode(results).write(to:outputURL,options:.withoutOverwriting)
    print("Wrote \(results.count) measurements. No finish labels changed.")
}
