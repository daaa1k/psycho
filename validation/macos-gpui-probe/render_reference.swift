// Independent CoreText oracle: shape at 1280x720, then uniformly scale glyph positions.
import Foundation
import CoreText
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

struct Item: Decodable {
    let slide: Int, id: String, kind: String, lines: [String], font: String
    let x: Double, y: Double, width: Double, height: Double, font_size: Double, line_height: Double
}
let args = CommandLine.arguments
let items = try JSONDecoder().decode([Item].self, from: Data(contentsOf: URL(fileURLWithPath: args[1])))
let output = URL(fileURLWithPath: args[2], isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
let modes: [(String,Int,Int,Double,Double,Double)] = [
    ("small",1024,792,0.7555555555555555,28.444444444444457,176),
    ("large",1440,992,1.0333333333333334,58.66666666666663,176),
    ("fullscreen",1920,1200,1.5,0,60)
]
var rows: [[String:Any]] = []
for (mode,w,h,scale,ox,oy) in modes {
 for slide in 1...2 {
    let context = CGContext(data:nil,width:w,height:h,bitsPerComponent:8,bytesPerRow:w*4,
        space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(gray:1,alpha:1); context.fill(CGRect(x:0,y:0,width:w,height:h))
    context.setFillColor(gray:0,alpha:1)
    context.setShouldAntialias(true)
    context.setAllowsFontSubpixelPositioning(true)
    context.setShouldSubpixelPositionFonts(true)
    context.setAllowsFontSubpixelQuantization(false)
    context.setShouldSubpixelQuantizeFonts(false)
    for item in items where item.slide == slide {
        if item.kind == "code" || item.kind == "image" {
            let rgb: [Double] = item.kind == "code" ? [241,242,243] : [229,232,235]
            context.setFillColor(red:rgb[0]/255,green:rgb[1]/255,blue:rgb[2]/255,alpha:1)
            context.fill(CGRect(x:ox+item.x*scale,y:Double(h)-oy-(item.y+item.height)*scale,width:item.width*scale,height:item.height*scale))
        }
        if item.kind == "caption" { context.setFillColor(red:76.0/255,green:81.0/255,blue:87.0/255,alpha:1) }
        else { context.setFillColor(gray:34.0/255,alpha:1) }

        let font = CTFontCreateWithName(item.font as CFString, item.font_size, nil)
        let ascent = CTFontGetAscent(font), descent = CTFontGetDescent(font)
        let baseline = (item.line_height - ascent - descent)/2 + ascent
        for (lineIndex,text) in item.lines.enumerated() {
            let line = CTLineCreateWithAttributedString(NSAttributedString(string:text,
                attributes:[NSAttributedString.Key(kCTFontAttributeName as String):font]))
            let lineWidth = CTLineGetTypographicBounds(line,nil,nil,nil)
            let dx = item.kind == "code" ? 16.0 : item.kind == "image" ? (item.width-lineWidth)/2 : 0
            let dy = item.kind == "code" ? 16.0 : item.kind == "image" ? (item.height-item.line_height)/2 : 0
            for run in CTLineGetGlyphRuns(line) as! [CTRun] {
                let attrs = CTRunGetAttributes(run) as NSDictionary
                let runFont = attrs[kCTFontAttributeName] as! CTFont
                let scaledFont = CTFontCreateCopyWithAttributes(runFont, CTFontGetSize(runFont)*scale,nil,nil)
                let count = CTRunGetGlyphCount(run)
                var glyphs = [CGGlyph](repeating:0,count:count)
                var positions = [CGPoint](repeating:.zero,count:count)
                var indices = [CFIndex](repeating:0,count:count)
                CTRunGetGlyphs(run,CFRange(location:0,length:0),&glyphs)
                CTRunGetPositions(run,CFRange(location:0,length:0),&positions)
                CTRunGetStringIndices(run,CFRange(location:0,length:0),&indices)
                for i in 0..<count {
                    var glyph = glyphs[i]
                    var box = CGRect.zero
                    CTFontGetBoundingRectsForGlyphs(scaledFont,.horizontal,&glyph,&box,1)
                    let x = ox+(item.x+dx+positions[i].x)*scale
                    let y = oy+(item.y+dy+Double(lineIndex)*item.line_height+baseline+positions[i].y)*scale
                    var position = CGPoint(x:x,y:Double(h)-y)
                    CTFontDrawGlyphs(scaledFont,&glyph,&position,1,context)
                    if box.width > 0 && box.height > 0 {
                        rows.append(["mode":mode,"slide":slide,"element":item.id,"line":lineIndex,
                            "index":indices[i],"glyph":Int(glyph),"font":CTFontCopyPostScriptName(runFont) as String,
                            "x":x,"y":y,"left":x+box.minX,"top":y-box.maxY,
                            "right":x+box.maxX,"bottom":y-box.minY])
                    }
                }
            }
        }
    }
    let url=output.appendingPathComponent("reference-\(mode)-1-\(slide).png")
    let dest=CGImageDestinationCreateWithURL(url as CFURL,UTType.png.identifier as CFString,1,nil)!
    CGImageDestinationAddImage(dest,context.makeImage()!,nil)
    precondition(CGImageDestinationFinalize(dest))
 }
}
try JSONSerialization.data(withJSONObject:rows,options:[.prettyPrinted,.sortedKeys])
    .write(to:output.appendingPathComponent("glyphs.json"))
print("reference glyphs: \(rows.count)")
