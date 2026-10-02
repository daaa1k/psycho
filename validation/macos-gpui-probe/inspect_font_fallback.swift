import CoreText
import Foundation

for family in ["Hiragino Sans", "Menlo"] {
    let base = CTFontCreateWithName(family as CFString, 18, nil)
    print("requested=\(family) resolved=\(CTFontCopyPostScriptName(base))")
    for sample in ["日", "a", "🌟"] {
        let selected = CTFontCreateForString(base, sample as CFString, CFRange(location: 0, length: (sample as NSString).length))
        print("  sample=\(sample) fallback=\(CTFontCopyPostScriptName(selected))")
    }
}
