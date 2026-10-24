import Darwin
// Schnauz's direct Nextcloud client. No Nextcloud implementation is embedded.
import Foundation
import Security

enum NCFailure: LocalizedError {
  case message(String)
  var errorDescription: String? {
    if case .message(let text) = self { return text }
    return nil
  }
}

struct NCAccount: Codable {
  let id: String
  let root: String
  let server: String
  let login: String
  let userID: String
  let remoteRoot: String
  var locking: Bool
}

struct NCConfiguration: Codable {
  var version = 1
  var accounts: [NCAccount] = []
}

enum NCPaths {
  static let unreserved = CharacterSet(
    charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~")
  static func encode(_ value: String) -> String {
    value.addingPercentEncoding(withAllowedCharacters: unreserved)!
  }
  static func server(_ input: String) throws -> URL {
    let text = input.trimmingCharacters(in: .whitespacesAndNewlines)
    guard var parts = URLComponents(string: text.contains("://") ? text : "https://" + text),
      ["https", "http"].contains(parts.scheme?.lowercased() ?? ""),
      let host = parts.host, !host.isEmpty, parts.user == nil, parts.password == nil,
      parts.query == nil, parts.fragment == nil
    else {
      throw NCFailure.message(
        "Enter the Nextcloud server address, including its subfolder if it has one.")
    }
    parts.scheme = parts.scheme?.lowercased()
    while parts.percentEncodedPath.hasSuffix("/") { parts.percentEncodedPath.removeLast() }
    guard let url = parts.url else { throw NCFailure.message("The server address is invalid.") }
    return url
  }
  static func sameOrigin(_ a: URL, _ b: URL) -> Bool {
    a.scheme?.lowercased() == b.scheme?.lowercased() && a.host?.lowercased() == b.host?.lowercased()
      && (a.port ?? (a.scheme == "https" ? 443 : 80))
        == (b.port ?? (b.scheme == "https" ? 443 : 80))
      && b.user == nil && b.password == nil
  }
  static func endpoint(_ base: URL, _ segments: [String], query: [URLQueryItem] = []) -> URL {
    var parts = URLComponents(url: base, resolvingAgainstBaseURL: false)!
    parts.percentEncodedPath = parts.percentEncodedPath.trimmingCharacters(
      in: CharacterSet(charactersIn: "/"))
    parts.percentEncodedPath =
      "/"
      + ([parts.percentEncodedPath].filter { !$0.isEmpty } + segments.map(encode)).joined(
        separator: "/")
    parts.queryItems = query.isEmpty ? nil : query
    return parts.url!
  }
  static func remoteFolder(_ value: String) throws -> String {
    let pieces = value.split(separator: "/", omittingEmptySubsequences: true).map(String.init)
    guard pieces.allSatisfy({ $0 != ".." && $0 != "." && !$0.contains("\0") }) else {
      throw NCFailure.message("The remote folder must not contain dot or parent components.")
    }
    return "/" + pieces.joined(separator: "/")
  }
  static func remotePath(_ local: String, account: NCAccount) throws -> String {
    let root = URL(fileURLWithPath: account.root).standardizedFileURL.pathComponents
    let path = URL(fileURLWithPath: local).standardizedFileURL.pathComponents
    guard path.starts(with: root) else {
      throw NCFailure.message("This item is outside the connected Nextcloud folder.")
    }
    let prefix = try remoteFolder(account.remoteRoot)
    let suffix = path.dropFirst(root.count).joined(separator: "/")
    return (prefix == "/" ? "" : prefix)
      + (suffix.isEmpty ? (prefix == "/" ? "/" : "") : "/" + suffix)
  }
  static func form(_ fields: [String: String]) -> Data {
    fields.sorted { $0.key < $1.key }.map { encode($0.key) + "=" + encode($0.value) }.joined(
      separator: "&"
    ).data(using: .utf8)!
  }
  static func webLink(_ text: String) throws -> URL {
    guard let url = URL(string: text), ["http", "https"].contains(url.scheme?.lowercased() ?? ""),
      url.host != nil, url.user == nil, url.password == nil
    else { throw NCFailure.message("The server returned an invalid link.") }
    return url
  }
}

final class NCTransport: NSObject, URLSessionTaskDelegate {
  let session: URLSession
  // Refuse redirects, including same-origin ones, so no authenticated request
  // or login polling token is ever forwarded to a second endpoint implicitly.
  // A canonical server address can be entered during setup instead.
  init(configuration: URLSessionConfiguration = .ephemeral) {
    configuration.timeoutIntervalForRequest = 25
    configuration.timeoutIntervalForResource = 40
    configuration.httpCookieStorage = nil
    configuration.urlCredentialStorage = nil
    session = URLSession(configuration: configuration)
    super.init()
  }
  func request(
    _ url: URL, method: String = "GET", body: Data? = nil, headers: [String: String] = [:],
    credentials: (String, String)? = nil
  ) async throws -> (Data, Int) {
    var request = URLRequest(url: url)
    request.httpMethod = method
    request.httpBody = body
    request.setValue("Schnauz", forHTTPHeaderField: "User-Agent")
    request.setValue("true", forHTTPHeaderField: "OCS-APIRequest")
    for (name, value) in headers { request.setValue(value, forHTTPHeaderField: name) }
    if let (login, password) = credentials {
      let encoded = Data((login + ":" + password).utf8).base64EncodedString()
      request.setValue("Basic " + encoded, forHTTPHeaderField: "Authorization")
    }
    do {
      let (data, response) = try await session.data(for: request, delegate: self)
      guard let response = response as? HTTPURLResponse, data.count <= 8 * 1024 * 1024 else {
        throw NCFailure.message("The server response is invalid or too large.")
      }
      return (data, response.statusCode)
    } catch is CancellationError { throw CancellationError() } catch let error as NCFailure {
      throw error
    } catch let error as URLError {
      if error.code == .cancelled { throw CancellationError() }
      switch error.code {
      case .serverCertificateUntrusted, .serverCertificateHasBadDate,
        .serverCertificateHasUnknownRoot, .secureConnectionFailed:
        throw NCFailure.message(
          "The server's TLS certificate is not trusted. Use its valid HTTPS address or configure certificate trust in macOS."
        )
      case .timedOut: throw NCFailure.message("The Nextcloud request timed out. Please try again.")
      default:
        throw NCFailure.message("Cannot reach Nextcloud. Check the connection and server address.")
      }
    }
  }
  func urlSession(
    _ session: URLSession, task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
    completionHandler: @escaping (URLRequest?) -> Void
  ) {
    completionHandler(nil)
  }
  static func check(_ status: Int) throws {
    switch status {
    case 200..<300: return
    case 301..<400:
      throw NCFailure.message(
        "The server redirected this request. Connect using its final Nextcloud address.")
    case 401:
      throw NCFailure.message("Nextcloud access has expired or was revoked. Reconnect this folder.")
    case 403:
      throw NCFailure.message("Nextcloud does not permit this action for this account or item.")
    case 404:
      throw NCFailure.message(
        "The item or server feature is unavailable. Check the remote folder mapping and whether the item has finished syncing."
      )
    case 409, 412, 423:
      throw NCFailure.message(
        "Nextcloud could not change this lock. The item may be locked by another user or application."
      )
    case 429:
      throw NCFailure.message("Nextcloud is limiting requests. Wait briefly before trying again.")
    default: throw NCFailure.message("Nextcloud could not finish the request (HTTP \(status)).")
    }
  }
}

final class NCDAVParser: NSObject, XMLParserDelegate {
  private var text = "", block: [String: String] = [:], status = ""
  private var inProperties = false
  var values: [String: String] = [:]
  func parser(
    _ parser: XMLParser, didStartElement name: String, namespaceURI: String?,
    qualifiedName: String?, attributes: [String: String]
  ) {
    text = ""
    if name == "propstat" {
      block = [:]
      status = ""
    }
    if name == "prop" { inProperties = true }
    if name == "collection" && namespaceURI == "DAV:" && inProperties {
      block["collection"] = "true"
    }
  }
  func parser(_ parser: XMLParser, foundCharacters string: String) { text += string }
  func parser(
    _ parser: XMLParser, didEndElement name: String, namespaceURI: String?, qualifiedName: String?
  ) {
    if inProperties
      && ((namespaceURI == "http://owncloud.org/ns" && ["fileid", "privatelink"].contains(name))
        || (namespaceURI == "http://nextcloud.org/ns"
          && ["lock", "lock-owner", "lock-owner-displayname", "lock-owner-type"].contains(name)))
    {
      block[name] = text.trimmingCharacters(in: .whitespacesAndNewlines)
    }
    if name == "prop" { inProperties = false }
    if name == "status" { status = text }
    if name == "propstat", status.split(separator: " ").dropFirst().first == "200" {
      values.merge(block) { _, new in new }
    }
    text = ""
  }
  static func parse(_ data: Data) throws -> [String: String] {
    let delegate = NCDAVParser()
    let parser = XMLParser(data: data)
    parser.shouldProcessNamespaces = true
    parser.shouldResolveExternalEntities = false
    parser.delegate = delegate
    guard parser.parse() else {
      throw NCFailure.message("Nextcloud returned invalid item metadata.")
    }
    return delegate.values
  }
}

struct NCShare {
  let id: String, recipient: String
  let type: Int, permissions: Int
  let url: String?
  init?(_ json: [String: Any]) {
    guard let id = (json["id"] as? String) ?? (json["id"] as? NSNumber)?.stringValue else {
      return nil
    }
    self.id = id
    type =
      (json["share_type"] as? NSNumber)?.intValue ?? Int(json["share_type"] as? String ?? "") ?? -1
    permissions =
      (json["permissions"] as? NSNumber)?.intValue ?? Int(json["permissions"] as? String ?? "") ?? 1
    recipient =
      type == 3
      ? ((json["label"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "Public link")
      : (json["share_with_displayname"] as? String ?? json["share_with"] as? String ?? "Shared item")
    url = json["url"] as? String
  }
}

// A write and the following list refresh are separate operations. In particular,
// a successful POST must not become a failed/retryable POST when its GET fails.
struct NCShareWorkflow {
  private(set) var pending = false
  private(set) var requiresRefresh = true
  private(set) var changeConfirmed = false

  var canChange: Bool { !pending && !requiresRefresh }
  var canRefresh: Bool { !pending }

  mutating func begin(changing: Bool) -> Bool {
    guard !pending, !changing || !requiresRefresh else { return false }
    pending = true
    if changing {
      // Also require a read after an uncertain write outcome, such as a lost
      // network response. Never encourage blindly submitting the write again.
      requiresRefresh = true
      changeConfirmed = false
    }
    return true
  }
  mutating func confirmedChange() { changeConfirmed = true }
  mutating func refreshed() { requiresRefresh = false }
  mutating func finish() { pending = false }
}

final class NCClient {
  let account: NCAccount, password: String, transport: NCTransport
  let base: URL
  init(account: NCAccount, password: String, transport: NCTransport = NCTransport()) throws {
    self.account = account
    self.password = password
    self.transport = transport
    base = try NCPaths.server(account.server)
  }
  func ocs(_ segments: [String], method: String = "GET", fields: [String: String] = [:])
    async throws -> Any
  {
    var query = [URLQueryItem(name: "format", value: "json")]
    if method == "GET" { query += fields.map { URLQueryItem(name: $0.key, value: $0.value) } }
    let url = NCPaths.endpoint(base, ["ocs", "v2.php"] + segments, query: query)
    return try await ocsURL(url, method: method, fields: method == "GET" ? [:] : fields)
  }
  func ocsURL(_ url: URL, method: String, fields: [String: String]) async throws -> Any {
    let (data, status) = try await transport.request(
      url, method: method, body: fields.isEmpty ? nil : NCPaths.form(fields),
      headers: ["Content-Type": "application/x-www-form-urlencoded", "Accept": "application/json"],
      credentials: (account.login, password))
    try NCTransport.check(status)
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
      let ocs = object["ocs"] as? [String: Any], let meta = ocs["meta"] as? [String: Any]
    else { throw NCFailure.message("Nextcloud returned an invalid API response.") }
    let code =
      (meta["statuscode"] as? NSNumber)?.intValue ?? Int(meta["statuscode"] as? String ?? "") ?? 0
    guard code == 100 || (200..<300).contains(code) else {
      if code == 401 || code == 403 || code == 404 { try NCTransport.check(code) }
      throw NCFailure.message(
        "Nextcloud rejected this action. Check the item's permissions and server settings.")
    }
    return ocs["data"] ?? NSNull()
  }
  func metadata(local: String) async throws -> [String: String] {
    let remote = try NCPaths.remotePath(local, account: account)
    let url = NCPaths.endpoint(
      base,
      ["remote.php", "dav", "files", account.userID] + remote.split(separator: "/").map(String.init)
    )
    let xml = """
      <?xml version="1.0"?><d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns"><d:prop><oc:fileid/><oc:privatelink/><d:resourcetype/><nc:lock/><nc:lock-owner/><nc:lock-owner-displayname/><nc:lock-owner-type/></d:prop></d:propfind>
      """
    let (data, status) = try await transport.request(
      url, method: "PROPFIND", body: Data(xml.utf8),
      headers: ["Depth": "0", "Content-Type": "application/xml; charset=utf-8"],
      credentials: (account.login, password))
    try NCTransport.check(status)
    let values = try NCDAVParser.parse(data)
    guard let id = values["fileid"], !id.isEmpty, id.utf8.allSatisfy({ (48...57).contains($0) })
    else {
      throw NCFailure.message(
        "This item is not available on the server yet, or the remote folder mapping is incorrect.")
    }
    return values
  }
  func internalLink(local: String) async throws -> URL {
    let values = try await metadata(local: local)
    if let link = values["privatelink"], !link.isEmpty { return try NCPaths.webLink(link) }
    return NCPaths.endpoint(base, ["f", values["fileid"]!])
  }
  func shares(local: String) async throws -> [NCShare] {
    let result = try await ocs(
      ["apps", "files_sharing", "api", "v1", "shares"],
      fields: [
        "path": NCPaths.remotePath(local, account: account), "reshares": "false",
        "subfiles": "false",
      ])
    guard let rows = result as? [[String: Any]] else {
      throw NCFailure.message("Nextcloud returned an invalid share list.")
    }
    return rows.compactMap(NCShare.init)
  }
  func createShare(
    local: String, type: Int, recipient: String = "", permissions: Int = 1,
    linkPassword: String = "", expiry: String = ""
  ) async throws {
    guard [0, 1, 3].contains(type),
      type == 3 || !recipient.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    else { throw NCFailure.message("Enter the Nextcloud user or group ID.") }
    var fields = [
      "path": try NCPaths.remotePath(local, account: account), "shareType": String(type),
      "permissions": String(type == 3 ? 1 : permissions), "sendMail": "false",
    ]
    if type != 3 { fields["shareWith"] = recipient }
    if !linkPassword.isEmpty { fields["password"] = linkPassword }
    if !expiry.isEmpty { fields["expireDate"] = expiry }
    _ = try await ocs(
      ["apps", "files_sharing", "api", "v1", "shares"], method: "POST", fields: fields)
  }
  func changeShare(_ share: NCShare, permissions: Int?) async throws {
    guard !share.id.isEmpty, share.id.utf8.allSatisfy({ (48...57).contains($0) }) else {
      throw NCFailure.message("The share identifier is invalid.")
    }
    _ = try await ocs(
      ["apps", "files_sharing", "api", "v1", "shares", share.id],
      method: permissions == nil ? "DELETE" : "PUT",
      fields: permissions.map { ["permissions": String($0)] } ?? [:])
  }
  func setLock(local: String, locked: Bool) async throws {
    let capabilities = try await ocs(["cloud", "capabilities"]) as? [String: Any]
    guard Self.supportsLocking(capabilities ?? [:]) else {
      throw NCFailure.message(
        "File locking is not enabled on this server. Its administrator must enable Nextcloud's Files Locking app."
      )
    }
    let values = try await metadata(local: local)
    guard values["collection"] != "true" else {
      throw NCFailure.message("Nextcloud file locking is available for files, not folders.")
    }
    if !locked {
      guard values["lock-owner"] == account.userID else {
        throw NCFailure.message(
          "Schnauz can only release locks confirmed as yours. This lock belongs to another user or has no owner information."
        )
      }
      guard values["lock-owner-type"] == "0" else {
        throw NCFailure.message(
          "This lock belongs to an application or another client, or its type is unknown. Remove it through its owner."
        )
      }
    }
    _ = try await ocs(
      ["apps", "files_lock", "lock", values["fileid"]!], method: locked ? "PUT" : "DELETE")
  }
  static func supportsLocking(_ payload: [String: Any]) -> Bool {
    let all = payload["capabilities"] as? [String: Any]
    let files = all?["files"] as? [String: Any]
    if let version = files?["locking"] as? String { return !version.isEmpty && version != "0" }
    return (files?["locking"] as? NSNumber)?.boolValue == true
  }
}

enum NCKeychain {
  static let service = "local.schnauz.nextcloud"
  static func store(_ password: String, account: NCAccount) throws {
    let data = try JSONSerialization.data(withJSONObject: [
      "password": password, "server": account.server, "login": account.login,
    ])
    let query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
      kSecAttrAccount as String: account.id, kSecValueData as String: data,
      kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
    ]
    let result = SecItemAdd(query as CFDictionary, nil)
    guard result == errSecSuccess else {
      throw NCFailure.message("Could not save Nextcloud access in macOS Keychain (\(result)).")
    }
  }
  static func read(_ account: NCAccount) throws -> String {
    let query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
      kSecAttrAccount as String: account.id, kSecReturnData as String: true,
      kSecMatchLimit as String: kSecMatchLimitOne,
    ]
    var value: CFTypeRef?
    guard SecItemCopyMatching(query as CFDictionary, &value) == errSecSuccess,
      let data = value as? Data,
      let stored = try JSONSerialization.jsonObject(with: data) as? [String: String],
      stored["server"] == account.server, stored["login"] == account.login,
      let password = stored["password"]
    else {
      throw NCFailure.message(
        "Nextcloud access is unavailable in Keychain or does not match this server. Reconnect this folder."
      )
    }
    return password
  }
  static func remove(_ id: String) {
    SecItemDelete(
      [
        kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
        kSecAttrAccount as String: id,
      ] as CFDictionary)
  }
}

enum NCStore {
  static func validate(_ config: NCConfiguration) throws {
    guard config.version == 1, Set(config.accounts.map(\.root)).count == config.accounts.count,
      Set(config.accounts.map(\.id)).count == config.accounts.count,
      config.accounts.allSatisfy({
        !$0.root.isEmpty && $0.root.hasPrefix("/") && UUID(uuidString: $0.id) != nil
      })
    else {
      throw NCFailure.message("Nextcloud settings are invalid. The file has been preserved.")
    }
    for account in config.accounts {
      _ = try NCPaths.server(account.server)
      _ = try NCPaths.remoteFolder(account.remoteRoot)
    }
  }
  static func load(_ url: URL) throws -> NCConfiguration {
    if !FileManager.default.fileExists(atPath: url.path) { return NCConfiguration() }
    let data = try Data(contentsOf: url)
    guard data.count <= 1024 * 1024 else {
      throw NCFailure.message("Nextcloud settings are too large.")
    }
    let config = try JSONDecoder().decode(NCConfiguration.self, from: data)
    try validate(config)
    return config
  }
  static func update(_ url: URL, change: (inout NCConfiguration) throws -> Void) throws {
    let fm = FileManager.default
    try fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    let descriptor = open(url.path + ".lock", O_CREAT | O_RDWR | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw NCFailure.message("Cannot lock Nextcloud settings.") }
    defer {
      flock(descriptor, LOCK_UN)
      close(descriptor)
    }
    guard flock(descriptor, LOCK_EX) == 0 else {
      throw NCFailure.message("Cannot lock Nextcloud settings.")
    }
    var config = try load(url)
    try change(&config)
    try validate(config)
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    try encoder.encode(config).write(to: url, options: [.atomic])
    try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
  }
}

struct NCLogin {
  let server: URL, login: URL, poll: URL, token: String
  static func begin(server: URL, transport: NCTransport) async throws -> NCLogin {
    let (data, status) = try await transport.request(
      NCPaths.endpoint(server, ["index.php", "login", "v2"]), method: "POST")
    try NCTransport.check(status)
    guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
      let poll = json["poll"] as? [String: String], let token = poll["token"], !token.isEmpty,
      let endpoint = poll["endpoint"].flatMap(URL.init(string:)),
      let login = (json["login"] as? String).flatMap(URL.init(string:)),
      NCPaths.sameOrigin(server, endpoint), NCPaths.sameOrigin(server, login)
    else { throw NCFailure.message("Nextcloud returned an invalid login response.") }
    return NCLogin(server: server, login: login, poll: endpoint, token: token)
  }
  func wait(transport: NCTransport, pollDelay: UInt64 = 2_000_000_000) async throws -> (
    URL, String, String
  ) {
    let deadline = Date().addingTimeInterval(20 * 60)
    while Date() < deadline {
      try Task.checkCancellation()
      let (data, status) = try await transport.request(
        poll, method: "POST", body: NCPaths.form(["token": token]),
        headers: ["Content-Type": "application/x-www-form-urlencoded"])
      if status == 404 {
        try await Task.sleep(nanoseconds: pollDelay)
        continue
      }
      try NCTransport.check(status)
      guard let value = try JSONSerialization.jsonObject(with: data) as? [String: String],
        let returned = value["server"],
        let login = value["loginName"], !login.isEmpty, !login.contains(":"),
        let password = value["appPassword"], !password.isEmpty
      else { throw NCFailure.message("Nextcloud returned incomplete login details.") }
      let canonical = try NCPaths.server(returned)
      guard NCPaths.sameOrigin(server, canonical) else {
        throw NCFailure.message(
          "The login server changed. Please connect using its canonical address.")
      }
      return (canonical, login, password)
    }
    throw NCFailure.message("Nextcloud login expired. Please connect again.")
  }
}
