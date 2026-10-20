import Foundation

struct NetworkLocation: Codable, Equatable {
    var url: String
    var username: String
    var guest: Bool
    var read_only: Bool
    var mount_root: String
    var relative: String
}
struct NetworkRequest: Codable { var mode: String; var location: NetworkLocation? }
struct ConnectedFolder: Codable { var location: NetworkLocation; var path: String }
struct NetworkResponse: Codable {
    var cancelled = false
    var folders: [ConnectedFolder] = []
    var error: String?
    var warning: String?
}
enum NetworkFailure: Error, LocalizedError {
    case address, relative, authentication, readOnly, connection(Int32), keychain(Int32)
    var errorDescription: String? {
        switch self {
        case .address: return "Enter an SMB or AFP server name, IP address or share URL without embedded credentials."
        case .relative: return "The saved folder is outside its network share."
        case .readOnly: return "This share is already mounted with write access. Eject it in Finder before connecting as read-only."
        case .authentication: return "Enter a username, or choose Guest."
        case .connection(let code): return "Could not connect to the network share (error \(code)). Check the server, share and login."
        case .keychain: return "Schnauz could not save or read this connection's password in Keychain."
        }
    }
}
func normalizedAddress(_ text: String, scheme: String) throws -> URL {
    let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty, !text.contains(where: { $0.isNewline || $0 == "\0" }),
          var value = URLComponents(string: text.contains("://") ? text : "\(scheme)://\(text)"),
          ["smb", "afp"].contains(value.scheme?.lowercased() ?? ""),
          let host = value.host, !host.isEmpty,
          value.user == nil, value.password == nil, value.query == nil, value.fragment == nil,
          value.port == nil || (1...65535).contains(value.port!) else { throw NetworkFailure.address }
    value.scheme = value.scheme!.lowercased(); value.host = host.lowercased()
    guard let url = value.url else { throw NetworkFailure.address }
    return url
}
func relativePath(_ selected: String, root: String) throws -> String {
    let selected = URL(fileURLWithPath: selected).standardizedFileURL.path
    let root = URL(fileURLWithPath: root).standardizedFileURL.path
    if selected == root { return "" }
    guard selected.hasPrefix(root + "/") else { throw NetworkFailure.relative }
    return String(selected.dropFirst(root.count + 1))
}
func resolvedPath(root: String, relative: String) throws -> String {
    guard !relative.hasPrefix("/"), !relative.contains("\0"),
          !relative.split(separator: "/").contains("..") else { throw NetworkFailure.relative }
    let path = URL(fileURLWithPath: root).appendingPathComponent(relative).standardizedFileURL.path
    _ = try relativePath(path, root: root)
    return path
}
func credentialAccount(url: URL, username: String) -> String {
    // An unambiguous key; never includes a password or another app's service.
    let values = [url.absoluteString, username]
    return (try! JSONEncoder().encode(values)).base64EncodedString()
}
