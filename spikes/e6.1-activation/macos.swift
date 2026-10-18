// E6.1 activation spike, macOS. Throwaway: it proves what the real
// FontActivator (E6.2+) can rely on; see docs/adr/0008-font-activation.md.
//
//   swiftc -O macos.swift -o /tmp/e61 && /tmp/e61 check Diplomata
//
// Modes (each runs as its own process; macos.sh drives them):
//   check <family>                       -> {"present":true|false}
//   watch <family> <ready> <seconds>     -> ms until a running process sees it
//   register <process|session|user> <path> <ready>   registers, stays alive until <ready> is deleted
//   unregister <session|user> <path>
import CoreText
import Foundation

func scope(_ s: String) -> CTFontManagerScope {
    switch s {
    case "process": return .process
    case "session": return .session
    case "user": return .persistent  // == kCTFontManagerScopeUser on macOS
    default: fatalError("scope \(s)")
    }
}

func present(_ family: String) -> Bool {
    let names = CTFontManagerCopyAvailableFontFamilyNames() as? [String] ?? []
    return names.contains(family)
}

func err(_ e: Unmanaged<CFError>?) -> String {
    guard let e = e?.takeRetainedValue() else { return "" }
    return CFErrorCopyDescription(e) as String
}

let a = CommandLine.arguments
switch a[1] {
case "check":
    print("{\"present\":\(present(a[2]))}")
case "watch":
    FileManager.default.createFile(atPath: a[3], contents: Data("up".utf8))
    let start = Date()
    var seen: Int? = nil
    while seen == nil && Date().timeIntervalSince(start) < Double(a[4])! {
        if present(a[2]) { seen = Int(Date().timeIntervalSince(start) * 1000) }
        // A running app hears about changes through its run loop.
        RunLoop.current.run(until: Date().addingTimeInterval(0.25))
    }
    print("{\"seen_ms\":\(seen.map(String.init) ?? "null")}")
case "register":
    var e: Unmanaged<CFError>?
    let ok = CTFontManagerRegisterFontsForURL(URL(fileURLWithPath: a[3]) as CFURL, scope(a[2]), &e)
    FileManager.default.createFile(atPath: a[4], contents: Data("\(ok) \(err(e))".utf8))
    while FileManager.default.fileExists(atPath: a[4]) { usleep(200_000) }
case "unregister":
    var e: Unmanaged<CFError>?
    let ok = CTFontManagerUnregisterFontsForURL(URL(fileURLWithPath: a[3]) as CFURL, scope(a[2]), &e)
    print("\(ok) \(err(e))")
default:
    fatalError("mode \(a[1])")
}
