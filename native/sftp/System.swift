import Foundation
import Security
import LocalAuthentication

final class KeychainStore: CredentialStore {
    private func query(_ account: String, _ service: String, interactive: Bool = false) -> [String: Any] {
        let context = LAContext()
        context.interactionNotAllowed = !interactive
        return [kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service, kSecAttrAccount as String: account,
                kSecUseAuthenticationContext as String: context]
    }
    func set(account: String, password: String) throws {
        // Writes follow an explicit Save/Connect click. Background reads never prompt.
        var key = query(account, "local.schnauz.sftp", interactive: true)
        let value = Data(password.utf8)
        var status = SecItemUpdate(key as CFDictionary, [kSecValueData as String: value] as CFDictionary)
        if status == errSecItemNotFound {
            key[kSecValueData as String] = value
            status = SecItemAdd(key as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            throw CredentialFailure(message: "Couldn't save the SFTP password in Schnauz's Keychain. Unlock Keychain and try again.")
        }
    }
    func get(account: String, legacyService: String?) throws -> String? {
        let legacy = legacyService == "shuffle-studio-sftp" ? legacyService : nil
        for service in ["local.schnauz.sftp", legacy].compactMap({ $0 }) {
            var key = query(account, service)
            key[kSecReturnData as String] = true
            key[kSecMatchLimit as String] = kSecMatchLimitOne
            var item: CFTypeRef?
            let status = SecItemCopyMatching(key as CFDictionary, &item)
            if status == errSecItemNotFound { continue }
            guard status == errSecSuccess, let data = item as? Data, let password = String(data: data, encoding: .utf8) else {
                throw CredentialFailure(message: "SFTP access is unavailable. Unlock Keychain and reconnect or re-enter the password.")
            }
            return password
        }
        return nil
    }
}
