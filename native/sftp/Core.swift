import Foundation

struct Request: Decodable {
    let operation: String
    let account: String
    let password: String?
    let legacy_service: String?
}
struct Response: Encodable {
    var success = false
    var password: String?
    var message: String?
}
struct CredentialFailure: Error { let message: String }
protocol CredentialStore {
    func set(account: String, password: String) throws
    func get(account: String, legacyService: String?) throws -> String?
}
func handle(_ request: Request, store: CredentialStore) -> Response {
    guard request.account.hasPrefix("sftp:"), request.account.utf8.count <= 2048 else {
        return Response(message: "Invalid SFTP credential account.")
    }
    do {
        if request.operation == "set", let password = request.password {
            guard password.utf8.count <= 65536 else { return Response(message: "The password is too long.") }
            try store.set(account: request.account, password: password)
            return Response(success: true)
        }
        guard request.operation == "get" else { return Response(message: "Unknown credential operation.") }
        let legacy = request.legacy_service == "shuffle-studio-sftp" ? request.legacy_service : nil
        return Response(success: true, password: try store.get(account: request.account, legacyService: legacy))
    } catch let failure as CredentialFailure {
        return Response(message: failure.message)
    } catch { return Response(message: "SFTP credential access failed.") }
}
