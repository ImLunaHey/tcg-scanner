import Foundation
import ImageIO
import ScannerAPI

/// Reads the separated 3/0 cheek shapes in the Pikachu mark, not card artwork or
/// foil reflectivity. Red ink and blue ink cover the yellow and rainbow designs.
enum AnniversaryStampDetector {
    struct Digit { let x: Int; let y: Int; let width: Int; let height: Int; let three: Double; let zero: Double }
    static func analyze(_ data: Data, orientation: Int) throws -> CardStampEvidence {
        guard (0..<4).contains(orientation), data.count <= 32 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, nil) else { throw ScannerError("image", "Invalid stamp image.") }
        let full = try CardStampDetector.raster(source, maximumSize: 1260).rotated(orientation)
        guard full.height >= 600, (0.65...0.80).contains(Double(full.width)/Double(full.height)) else {
            return evidence(.unsupported, orientation: orientation)
        }
        let small = try CardStampDetector.raster(source, maximumSize: 1050).rotated(orientation)
        let a = try match(full), b = try match(small)
        let score = min(a?.score ?? 0, b?.score ?? 0)
        let same = a != nil && b != nil && abs(a!.region.x - b!.region.x) < 0.025 && abs(a!.region.y - b!.region.y) < 0.025
        let state: CardStampEvidence.State = same && score >= 0.79 ? .present : (a?.score ?? 0) >= 0.75 ? .uncertain : .absent
        var result = evidence(state, orientation: orientation)
        result.region = state == .absent ? nil : a?.region
        result.similarity = score; result.baselineSimilarity = a?.score; result.downsampledSimilarity = b?.score
        return result
    }
    private static func evidence(_ state: CardStampEvidence.State, orientation: Int) -> CardStampEvidence {
        var value = CardStampEvidence(state: state, sourceQuarterTurns: orientation, evidence: [
            state == .present ? "Separated 3 and 0 cheek shapes agree at two resolutions in the Pikachu anniversary-mark region." : "The 30th anniversary cheek shapes were not established at both resolutions.",
            "Local shape evidence does not establish finish or authenticity; catalogue identity must independently agree."
        ])
        value.kind = "pikachu-30th-anniversary"; value.method = "anniversary-cheeks-v1"
        return value
    }
    private static func match(_ raster: CardStampDetector.Raster) throws -> (score: Double, region: ImageCrop)? {
        let w=raster.width, h=raster.height
        var best: (Double, ImageCrop)?
        for blue in [false,true] {
            func ink(_ index: Int) -> Bool {
                let i=index*4, r=Int(raster.pixels[i]), g=Int(raster.pixels[i+1]), b=Int(raster.pixels[i+2])
                return blue ? b > r+20 && b > g+10 : r > g+35 && r > b+35
            }
            var seen=[Bool](repeating: false, count: w*h), digits=[Digit]()
            let x0=Int(Double(w)*0.70), x1=Int(Double(w)*0.98)
            for y in Int(Double(h)*0.32)..<Int(Double(h)*0.94) {
                try Task.checkCancellation()
                for x in x0..<x1 {
                    let first=y*w+x
                    guard !seen[first], ink(first) else { continue }
                    var queue=[first], head=0, minX=x,maxX=x,minY=y,maxY=y; seen[first]=true
                    while head<queue.count {
                        let p=queue[head];head+=1;let px=p%w,py=p/w
                        minX=min(minX,px);maxX=max(maxX,px);minY=min(minY,py);maxY=max(maxY,py)
                        for (dx,dy) in [(1,0),(-1,0),(0,1),(0,-1)] {
                            let nx=px+dx,ny=py+dy,index=ny*w+nx
                            if nx>=x0 && nx<x1 && ny>=Int(Double(h)*0.32) && ny<Int(Double(h)*0.94) && !seen[index] && ink(index) {
                                seen[index]=true;queue.append(index)
                            }
                        }
                    }
                    let dw=maxX-minX+1,dh=maxY-minY+1
                    guard (0.012...0.045).contains(Double(dh)/Double(h)), (0.45...1.2).contains(Double(dw)/Double(dh)) else { continue }
                    let component=Set(queue)
                    let mask=(0..<576).map { cell in
                        let xx=minX + min(dw-1,Int((Double(cell%24)+0.5)*Double(dw)/24))
                        let yy=minY + min(dh-1,Int((Double(cell/24)+0.5)*Double(dh)/24))
                        return component.contains(yy*w+xx)
                    }
                    func similarity(_ template: [Bool]) -> Double {
                        let intersection=mask.indices.filter {mask[$0] && template[$0]}.count
                        let union=mask.indices.filter {mask[$0] || template[$0]}.count
                        return Double(intersection)/Double(max(1,union))
                    }
                    // A zero must have an enclosed counter. Similar rounded
                    // outlines alone confuse the older anniversary digit 5.
                    // Close one-pixel ink breaks caused by foil/scan texture;
                    // the open side of a 5 remains open at this glyph scale.
                    func neighbors(_ cell: Int) -> [Int] {
                        (-1...1).flatMap { dy in (-1...1).compactMap { dx in
                            let x=cell%24+dx,y=cell/24+dy
                            return (0..<24).contains(x) && (0..<24).contains(y) ? y*24+x : nil
                        } }
                    }
                    let expanded=mask.indices.map {neighbors($0).contains {mask[$0]}}
                    let closed=mask.indices.map {neighbors($0).allSatisfy {expanded[$0]}}
                    var outside=Set<Int>(), flood=[Int]()
                    for cell in 0..<576 where (cell/24 == 0 || cell/24 == 23 || cell%24 == 0 || cell%24 == 23) && !closed[cell] {
                        if outside.insert(cell).inserted { flood.append(cell) }
                    }
                    var index=0
                    while index<flood.count {
                        let cell=flood[index];index+=1
                        for (dx,dy) in [(1,0),(-1,0),(0,1),(0,-1)] {
                            let xx=cell%24+dx,yy=cell/24+dy,next=yy*24+xx
                            if (0..<24).contains(xx) && (0..<24).contains(yy) && !closed[next] && outside.insert(next).inserted {flood.append(next)}
                        }
                    }
                    let counter=Double(mask.indices.filter {!closed[$0] && !outside.contains($0)}.count)/576
                    let hasCounter=(0.008...0.35).contains(counter)
                    digits.append(Digit(x:minX,y:minY,width:dw,height:dh,three:similarity(blue ? AnniversaryCheeks.rainbowThree : AnniversaryCheeks.three),zero:hasCounter ? similarity(blue ? AnniversaryCheeks.rainbowZero : AnniversaryCheeks.zero) : 0))
                }
            }
            for left in digits where left.three >= 0.75 {
                for right in digits where right.zero >= 0.75 && right.x>left.x {
                    let height=Double(left.height+right.height)/2
                    guard abs(Double(left.y-right.y)) < height*0.35,
                          (0.75...1.3).contains(Double(left.height)/Double(right.height)),
                          (2.5...5.5).contains(Double(right.x-left.x)/Double(left.width)) else { continue }
                    let score=min(left.three,right.zero)
                    let region=ImageCrop(x:Double(left.x)/Double(w),y:Double(left.y)/Double(h),width:Double(right.x+right.width-left.x)/Double(w),height:Double(max(left.height,right.height))/Double(h))
                    if best == nil || score>best!.0 {best=(score,region)}
                }
            }
        }
        return best
    }
}

// Frozen cheek glyphs measured from B-001/C-003 (red) and C-002 (blue),
// excluding card artwork. Other saved cards provide held-out comparisons.
private enum AnniversaryCheeks {
    static let three: [Bool] = [
        ".........#######........",
        ".....##############.....",
        "...#################....",
        ".####################...",
        "#####################...",
        "#####################...",
        "#####################...",
        "#####################...",
        "########..##########....",
        ".###################....",
        "....#################...",
        "....##################..",
        ".....##################.",
        "......#######..#########",
        "...#...........#########",
        "..######....############",
        "..######################",
        "..######################",
        "..#####################.",
        "..####################..",
        "...###################..",
        "....################....",
        "......#############.....",
        "........########........",
    ].flatMap { $0.map { $0 == "#" } }
    static let zero: [Bool] = [
        "..........#######.......",
        ".......############.....",
        "......##############....",
        ".....################...",
        "...###################..",
        "...####################.",
        "..#####################.",
        ".######################.",
        ".###########..##########",
        ".###########..##########",
        ".##########....#########",
        ".########......#########",
        ".########......########.",
        ".########......########.",
        ".#########....#########.",
        "######################..",
        ".#####################..",
        ".####################...",
        ".###################....",
        "..##################....",
        "...################.....",
        "...##############.......",
        "....#############.......",
        "......########..........",
    ].flatMap { $0.map { $0 == "#" } }
    static let rainbowThree: [Bool] = [
        ".......#########........",
        ".....#############......",
        ".####################...",
        "#####################...",
        "#####################...",
        "#############.#######...",
        "#########......####.....",
        "#####.........####......",
        ".##........#######......",
        ".........#########......",
        ".....##############.....",
        ".....################...",
        "......#######...#######.",
        ".......##........######.",
        ".................######.",
        ".................#######",
        "..###...........########",
        ".######################.",
        "..#####################.",
        "..####################..",
        "..###################...",
        "...##################...",
        "......###########.......",
        "...........###..........",
    ].flatMap { $0.map { $0 == "#" } }
    static let rainbowZero: [Bool] = [
        "...........####.........",
        "........##########......",
        "......###############...",
        ".....################...",
        "....##################..",
        "...####################.",
        "..#####################.",
        ".##########.....########",
        ".#########......########",
        "##########.......#######",
        "#########........#######",
        "########.........#######",
        "#######..........#######",
        "#######..........#######",
        "#######.........#######.",
        "########........#######.",
        "########.......########.",
        "######################..",
        ".####################...",
        ".###################....",
        "..#################.....",
        "..#################.....",
        "....############........",
        "......########..........",
    ].flatMap { $0.map { $0 == "#" } }
}
