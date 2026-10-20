import AppKit
import NetFS
import Security

enum NetworkSystem {
    static let service = "local.schnauz.network"
    static func query(_ url: URL, _ username: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: credentialAccount(url: url, username: username)]
    }
    static func password(_ url: URL, _ username: String) throws -> String? {
        var query = query(url, username)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data, let value = String(data: data, encoding: .utf8) else { throw NetworkFailure.keychain(status) }
        return value
    }
    static func savePassword(_ password: String, url: URL, username: String) throws {
        let key = query(url, username)
        let values: [String: Any] = [kSecValueData as String: Data(password.utf8)]
        var status = SecItemUpdate(key as CFDictionary, values as CFDictionary)
        if status == errSecItemNotFound {
            var item = key
            item[kSecValueData as String] = Data(password.utf8)
            item[kSecAttrLabel as String] = "Schnauz network connection · \(url.host ?? "server")"
            status = SecItemAdd(item as CFDictionary, nil)
        }
        guard status == errSecSuccess else { throw NetworkFailure.keychain(status) }
    }
    static func mount(url: URL, username: String, password: String?, guest: Bool, readOnly: Bool) throws -> [(URL, String)] {
        let options = NSMutableDictionary(dictionary: ["UIOption": "AllowUI", "Guest": guest, "ForceNewSession": true])
        let mountOptions = NSMutableDictionary(dictionary: ["MountFlags": readOnly ? Int(MNT_RDONLY) : 0])
        var points: Unmanaged<CFArray>?
        let status = NetFSMountURLSync(url as CFURL, nil,
            guest ? nil : username as CFString,
            password.map { $0 as CFString },
            options as CFMutableDictionary, mountOptions as CFMutableDictionary, &points)
        if status == -128 { return [] }
        guard status == 0 else { throw NetworkFailure.connection(Int32(status)) }
        let paths = points?.takeRetainedValue() as? [String] ?? []
        return try paths.map { path in
            if readOnly {
                var info = statfs()
                guard statfs(path, &info) == 0, info.f_flags & UInt32(MNT_RDONLY) != 0 else {
                    throw NetworkFailure.readOnly
                }
            }
            let local = URL(fileURLWithPath: path)
            guard let raw = NetFSCopyURLForRemountingVolume(local as CFURL)?.takeRetainedValue() else { throw NetworkFailure.connection(Int32(EIO)) }
            var components = URLComponents(url: raw as URL, resolvingAgainstBaseURL: false)!
            components.user = nil; components.password = nil
            let clean = try normalizedAddress(components.string ?? "", scheme: url.scheme!)
            return (clean, local.standardizedFileURL.path)
        }
    }
}
