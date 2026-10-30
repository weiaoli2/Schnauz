// Fixture-only domain tests. No Security import, Keychain, process, window or server.
import Foundation
final class FakeStore: CredentialStore {
    var accounts: [String: String] = [:]
    var calls = 0
    var legacy: String?
    var fail = false
    func set(account: String, password: String) throws {
        calls += 1
        if fail { throw CredentialFailure(message: "Fixture save failure") }
        accounts[account] = password
    }
    func get(account: String, legacyService: String?) throws -> String? {
        calls += 1; legacy = legacyService
        return accounts[account]
    }
}
@main enum Tests {
    static func main() throws {
        let store = FakeStore()
        let value = "quotes \" and $()\\ and 素\nend"
        let save = Request(operation: "set", account: "sftp:alice@fixture.test", password: value, legacy_service: nil)
        precondition(handle(save, store: store).success)
        let read = Request(operation: "get", account: save.account, password: nil, legacy_service: "shuffle-studio-sftp")
        let response = handle(read, store: store)
        precondition(response.success && response.password == value && store.legacy == "shuffle-studio-sftp")
        let json = try JSONEncoder().encode(response)
        let object = try JSONSerialization.jsonObject(with: json) as! [String: Any]
        precondition(object["password"] as? String == value)
        _ = handle(Request(operation: "get", account: save.account, password: nil, legacy_service: "another-app"), store: store)
        precondition(store.legacy == nil)
        let calls = store.calls
        precondition(!handle(Request(operation: "get", account: "another-app", password: nil, legacy_service: nil), store: store).success)
        precondition(!handle(Request(operation: "delete", account: save.account, password: nil, legacy_service: nil), store: store).success)
        precondition(!handle(Request(operation: "set", account: save.account, password: String(repeating: "x", count: 65537), legacy_service: nil), store: store).success)
        precondition(store.calls == calls)
        store.fail = true
        let failure = handle(save, store: store)
        precondition(!failure.success && failure.password == nil && failure.message == "Fixture save failure")
        print("PASS SFTP credential routing, exact secret payloads, legacy allowlist, validation and failed saves; fixture store only")
    }
}
