import Foundation

struct Volume: Codable {
    let name: String
    let path: String
    let local: Bool
    let total: UInt64?
    let free: UInt64?
    let identity: String?
    let ejectable: Bool
}
struct EjectResponse: Encodable {
    let success: Bool
    let message: String
}
func canEject(path: String, local: Bool, internalDevice: Bool?, removable: Bool, ejectable: Bool, identity: String?) -> Bool {
    let url = URL(fileURLWithPath: path)
    return local && path != "/" && url.deletingLastPathComponent().path == "/Volumes"
        && !(identity ?? "").isEmpty && (internalDevice == false || removable || ejectable)
}
func ejectVolume(path: String, identity: String, mounted: [Volume], perform: (String) throws -> Void) -> EjectResponse {
    guard !identity.isEmpty, path != "/", URL(fileURLWithPath: path).deletingLastPathComponent().path == "/Volumes",
          let volume = mounted.first(where: { $0.path == path && $0.identity == identity }), volume.local, volume.ejectable else {
        return EjectResponse(success: false, message: "The drive changed, is no longer connected, or cannot be ejected. Refresh Devices and try again.")
    }
    do {
        try perform(path)
        return EjectResponse(success: true, message: "Ejected “\(volume.name)”")
    } catch {
        return EjectResponse(success: false, message: "Couldn't eject “\(volume.name)”: \(error.localizedDescription)")
    }
}
