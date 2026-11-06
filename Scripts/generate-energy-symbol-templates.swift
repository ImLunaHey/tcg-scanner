import AppKit
// Development-only generator. Runtime recognition uses compiled silhouettes.
// Usage: swift Scripts/generate-energy-symbol-templates.swift <svg-directory>
guard CommandLine.arguments.count == 2 else {
    fatalError("Provide the directory containing tcg-<type>.svg reference glyphs")
}
let root = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let names=["grass","fire","water","lightning","psychic","fighting","darkness","metal","fairy"]
print("// Normalized TCG symbol silhouettes. See Docs/energy-symbols.md for provenance.\n// Generated for local shape comparison; these are not card-printing references.\nenum EnergySymbolTemplates {\n    static let values: [(name: String, ratio: Double, rows: [String])] = [")
for name in names {
 guard let image=NSImage(contentsOf: root.appendingPathComponent("tcg-\(name).svg")) else { fatalError("Missing glyph: \(name)") }
 var rect=CGRect(origin:.zero,size:image.size)
 let cg=image.cgImage(forProposedRect:&rect,context:nil,hints:nil)!
 var pixels=[UInt8](repeating:255,count:192*192*4)
 pixels.withUnsafeMutableBytes { p in
  let c=CGContext(data:p.baseAddress,width:192,height:192,bitsPerComponent:8,bytesPerRow:768,space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue)!
  c.setFillColor(CGColor(gray:1,alpha:1));c.fill(CGRect(x:0,y:0,width:192,height:192));c.draw(cg,in:CGRect(x:0,y:0,width:192,height:192))
 }
 let points=(0..<192*192).filter { pixels[$0*4]<128 }
 let left=points.map{$0%192}.min()!,right=points.map{$0%192}.max()!
 let top=points.map{$0/192}.min()!,bottom=points.map{$0/192}.max()!
 let w=right-left+1,h=bottom-top+1
 let ratio=image.size.width/image.size.height*Double(w)/Double(h)
 print("        (\"\(name.capitalized)\", \(ratio), [")
 for y in 0..<24 {
  let py=top+Int((Double(y)+0.5)*Double(h)/24)
  let row: [Character]=(0..<24).map { x in
   let px=left+Int((Double(x)+0.5)*Double(w)/24)
   return pixels[(py*192+px)*4]<128 ? "#" : "."
  }
  print("            \"" + String(row) + "\",")
 }

 print("        ]),")
}
print("    ]\n}")
