// Offline protocol tests. Never contact a server or read/write macOS Keychain.
import Foundation

enum TestFailure: Error { case failed(String) }
func expect(_ condition: @autoclosure () throws -> Bool, _ message: String) throws {
  if try !condition() { throw TestFailure.failed(message) }
}
func rejects(_ operation: () throws -> Void) throws {
  do { try operation() } catch is NCFailure { return }
  throw TestFailure.failed("Expected a safe client error")
}
func rejectsAsync(_ operation: () async throws -> Void) async throws {
  do { try await operation() } catch is NCFailure { return }
  throw TestFailure.failed("Expected a safe client error")
}

final class MockProtocol: URLProtocol {
  static var handler: ((URLRequest) throws -> (Int, Data))!
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    do {
      let (status, data) = try Self.handler(request)
      client?.urlProtocol(
        self,
        didReceive: HTTPURLResponse(
          url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil)!,
        cacheStoragePolicy: .notAllowed)
      client?.urlProtocol(self, didLoad: data)
      client?.urlProtocolDidFinishLoading(self)
    } catch { client?.urlProtocol(self, didFailWithError: error) }
  }
  override func stopLoading() {}
}
func mocked(_ handler: @escaping (URLRequest) throws -> (Int, Data)) -> NCTransport {
  MockProtocol.handler = handler
  let config = URLSessionConfiguration.ephemeral
  config.protocolClasses = [MockProtocol.self]
  return NCTransport(configuration: config)
}
func json(_ value: Any) -> Data { try! JSONSerialization.data(withJSONObject: value) }
func ocs(_ value: Any = [:], code: Int = 200) -> Data {
  json(["ocs": ["meta": ["statuscode": code], "data": value]])
}
func account(root: String = "/cloud", remote: String = "/Projects") -> NCAccount {
  NCAccount(
    id: UUID().uuidString, root: root, server: "https://cloud.example/nextcloud",
    login: "oli@example.com", userID: "oliver", remoteRoot: remote, locking: true)
}
func body(_ request: URLRequest) -> String {
  if let data = request.httpBody { return String(decoding: data, as: UTF8.self) }
  guard let stream = request.httpBodyStream else { return "" }
  stream.open()
  defer { stream.close() }
  var data = Data()
  var bytes = [UInt8](repeating: 0, count: 1024)
  while stream.hasBytesAvailable {
    let size = stream.read(&bytes, maxLength: bytes.count)
    if size <= 0 { break }
    data.append(contentsOf: bytes.prefix(size))
  }
  return String(decoding: data, as: UTF8.self)
}
func form(_ request: URLRequest) -> [String: String] {
  Dictionary(
    uniqueKeysWithValues: body(request).split(separator: "&").map { field in
      let pair = field.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
      return (
        String(pair[0]).removingPercentEncoding!,
        pair.count == 2 ? String(pair[1]).removingPercentEncoding! : ""
      )
    })
}
func dav(id: String = "42", extra: String = "", failed: String = "") -> Data {
  Data(
    """
    <d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns"><d:response><d:propstat><d:prop><oc:fileid>\(id)</oc:fileid>\(extra)</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat><d:propstat><d:prop>\(failed)</d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response></d:multistatus>
    """.utf8)
}

@main struct NextcloudTests {
  static func paths() throws {
    let base = try NCPaths.server(" cloud.example/nextcloud/ ")
    try expect(base.absoluteString == "https://cloud.example/nextcloud", "Canonical base")
    for bad in [
      "file:///tmp", "https://u:p@cloud.example", "https://cloud.example/?password=x",
      "https://cloud.example/#x", "",
    ] {
      try rejects { _ = try NCPaths.server(bad) }
    }
    let url = NCPaths.endpoint(base, ["remote.php", "dav", "files", "a/b", "ü + &%.txt"])
    try expect(url.absoluteString.hasSuffix("/a%2Fb/%C3%BC%20%2B%20%26%25.txt"), "Segment escaping")
    try expect(
      String(decoding: NCPaths.form(["a+b": "x &%ü"]), as: UTF8.self) == "a%2Bb=x%20%26%25%C3%BC",
      "Form escaping")
    try expect(
      try NCPaths.remotePath("/cloud/a b.txt", account: account()) == "/Projects/a b.txt",
      "Folder mapping")
    try expect(
      try NCPaths.remotePath("/cloud", account: account(remote: "/")) == "/", "Root mapping")
    try rejects { _ = try NCPaths.remotePath("/cloud-copy/file", account: account()) }
    try rejects { _ = try NCPaths.remotePath("/cloud/../private/file", account: account()) }
    try rejects { _ = try NCPaths.remoteFolder("/a/../b") }
    try rejects { _ = try NCPaths.webLink("javascript:alert(1)") }
    try rejects { _ = try NCPaths.webLink("https://u:p@cloud.example/f/1") }
    try expect(
      !NCPaths.sameOrigin(base, URL(string: "http://cloud.example")!), "Reject TLS downgrade")
    try expect(
      !NCPaths.sameOrigin(base, URL(string: "https://cloud.example:444")!), "Origin includes port")
  }
  static func login() async throws {
    let server = try NCPaths.server("https://cloud.example/nextcloud")
    var calls = 0
    let transport = mocked { request in
      try expect(
        request.value(forHTTPHeaderField: "Authorization") == nil, "Login requests are anonymous")
      try expect(request.httpMethod == "POST", "Login uses POST")
      calls += 1
      if calls == 1 {
        try expect(request.url!.path == "/nextcloud/index.php/login/v2", "Login v2 path")
        return (
          200,
          json([
            "poll": [
              "token": "secret+ &", "endpoint": "https://cloud.example/nextcloud/login/v2/poll",
            ], "login": "https://cloud.example/nextcloud/login/v2/flow/123",
          ])
        )
      }
      try expect(form(request)["token"] == "secret+ &", "Poll token roundtrip")
      if calls == 2 { return (404, Data()) }
      return (
        200,
        json([
          "server": server.absoluteString, "loginName": "oli@example.com",
          "appPassword": "fixture-password",
        ])
      )
    }
    let flow = try await NCLogin.begin(server: server, transport: transport)
    let (_, login, password) = try await flow.wait(transport: transport, pollDelay: 0)
    try expect(
      calls == 3 && login == "oli@example.com" && password == "fixture-password",
      "Pending then approved login")
    let bad = mocked { _ in
      (
        200,
        json([
          "poll": ["token": "t", "endpoint": "https://other.example/poll"],
          "login": "https://cloud.example/login",
        ])
      )
    }
    try await rejectsAsync { _ = try await NCLogin.begin(server: server, transport: bad) }
    let switched = mocked { _ in
      (200, json(["server": "http://cloud.example", "loginName": "u", "appPassword": "p"]))
    }
    try await rejectsAsync { _ = try await flow.wait(transport: switched, pollDelay: 0) }
    let cancelled = Task { try await flow.wait(transport: transport, pollDelay: 0) }
    cancelled.cancel()
    do {
      _ = try await cancelled.value
      throw TestFailure.failed("Cancelled login continued")
    } catch is CancellationError {}
  }
  static func metadata() async throws {
    let xml = dav(
      extra:
        "<oc:privatelink>https://cloud.example/f/42?a=1&amp;b=2</oc:privatelink><d:resourcetype><d:collection/></d:resourcetype>",
      failed: "<oc:fileid>999</oc:fileid>")
    let transport = mocked { request in
      try expect(
        request.httpMethod == "PROPFIND" && request.value(forHTTPHeaderField: "Depth") == "0",
        "Only selected item metadata")
      try expect(
        request.url!.path == "/nextcloud/remote.php/dav/files/oliver/Projects/a &.txt",
        "Uses actual UID and remote mapping")
      try expect(
        request.value(forHTTPHeaderField: "Authorization") == "Basic "
          + Data("oli@example.com:fixture".utf8).base64EncodedString(),
        "App credential authentication")
      return (207, xml)
    }
    let client = try NCClient(account: account(), password: "fixture", transport: transport)
    let values = try await client.metadata(local: "/cloud/a &.txt")
    try expect(
      values["fileid"] == "42" && values["collection"] == "true", "Ignore unavailable properties")
    let link = try await client.internalLink(local: "/cloud/a &.txt")
    try expect(
      link.absoluteString == "https://cloud.example/f/42?a=1&b=2",
      "Private link decodes XML entities")
    let fallback = try NCClient(
      account: account(), password: "fixture", transport: mocked { _ in (207, dav()) })
    let fallbackURL = try await fallback.internalLink(local: "/cloud")
    try expect(
      fallbackURL.absoluteString == "https://cloud.example/nextcloud/f/42", "Numeric ID fallback")
    let invalid = try NCClient(
      account: account(), password: "fixture", transport: mocked { _ in (207, dav(id: "../bad")) })
    try await rejectsAsync { _ = try await invalid.metadata(local: "/cloud") }
    try rejects { _ = try NCDAVParser.parse(Data("<broken>".utf8)) }
  }
  static func sharing() async throws {
    var methods: [String] = []
    let transport = mocked { request in
      methods.append(request.httpMethod!)
      try expect(
        request.url!.path.hasPrefix("/nextcloud/ocs/v2.php/apps/files_sharing/api/v1/shares"),
        "OCS endpoint")
      try expect(request.value(forHTTPHeaderField: "OCS-APIRequest") == "true", "OCS header")
      if request.httpMethod == "GET" {
        let query = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!
        try expect(
          query.contains(URLQueryItem(name: "path", value: "/Projects/a +&.txt")),
          "Share query path escapes correctly")
        return (
          200,
          ocs(
            [["id": "7", "share_type": 3, "permissions": "1", "url": "https://cloud.example/s/x"]],
            code: 100)
        )
      }
      if request.httpMethod == "POST" {
        let fields = form(request)
        try expect(
          fields["path"] == "/Projects/a +&.txt" && fields["permissions"] == "1",
          "Public link always read only")
        try expect(
          fields["sendMail"] == "false" && fields["password"] == "a+ &%"
            && fields["expireDate"] == "2030-01-02", "Share options")
      } else {
        try expect(request.url!.path.hasSuffix("/shares/7"), "Only selected share is changed")
        if request.httpMethod == "PUT" {
          try expect(form(request)["permissions"] == "3", "Explicit permission change")
        }
      }
      return (200, ocs())
    }
    let client = try NCClient(account: account(), password: "fixture", transport: transport)
    let shares = try await client.shares(local: "/cloud/a +&.txt")
    try expect(
      methods == ["GET"] && shares.count == 1, "Opening shares never creates a public link")
    try await client.createShare(
      local: "/cloud/a +&.txt", type: 3, permissions: 31, linkPassword: "a+ &%",
      expiry: "2030-01-02")
    try await client.changeShare(shares[0], permissions: 3)
    try await client.changeShare(shares[0], permissions: nil)
    try expect(methods == ["GET", "POST", "PUT", "DELETE"], "Explicit share actions only")
    try await rejectsAsync { try await client.createShare(local: "/cloud/file", type: 0) }
    try await rejectsAsync {
      try await client.changeShare(NCShare(["id": "../7"])!, permissions: nil)
    }
    for status in [401, 403, 404, 429, 302] {
      let failed = try NCClient(
        account: account(), password: "fixture", transport: mocked { _ in (status, Data()) })
      try await rejectsAsync { _ = try await failed.shares(local: "/cloud") }
    }
    let rejected = try NCClient(
      account: account(), password: "fixture", transport: mocked { _ in (200, ocs(code: 403)) })
    try await rejectsAsync { _ = try await rejected.shares(local: "/cloud") }
  }
  static func shareWorkflow() async throws {
    var methods: [String] = []
    var failRefresh = false
    let transport = mocked { request in
      methods.append(request.httpMethod!)
      if request.httpMethod == "PROPFIND" { return (207, dav()) }
      if request.httpMethod == "GET" {
        if failRefresh { return (503, Data()) }
        return (200, ocs([["id": "17", "share_type": 3, "permissions": 1]]))
      }
      try expect(request.httpMethod == "POST", "Only an explicit create changes shares")
      return (200, ocs(["id": "17"]))
    }
    let client = try NCClient(account: account(), password: "fixture", transport: transport)
    var workflow = NCShareWorkflow()
    try expect(!workflow.canChange, "No mutations before the initial share list is loaded")
    try expect(!workflow.begin(changing: true), "An uninitialized sharing window cannot create shares")
    try expect(workflow.begin(changing: false), "Opening the window starts a read")
    _ = try await client.metadata(local: "/cloud/file")
    _ = try await client.shares(local: "/cloud/file")
    workflow.refreshed()
    workflow.finish()
    try expect(methods == ["PROPFIND", "GET"] && workflow.canChange,
      "Opening/listing shares never sends a write")

    try expect(workflow.begin(changing: true), "Explicit create starts once")
    try expect(!workflow.begin(changing: true), "A pending mutation cannot be submitted twice")
    try await client.createShare(local: "/cloud/file", type: 3)
    workflow.confirmedChange()
    failRefresh = true
    try await rejectsAsync { _ = try await client.shares(local: "/cloud/file") }
    workflow.finish()
    let afterWrite = methods
    try expect(workflow.changeConfirmed && workflow.requiresRefresh,
      "A failed GET preserves the successful write outcome")
    try expect(!workflow.begin(changing: true) && methods == afterWrite,
      "A confirmed write with failed refresh cannot be submitted again")

    try expect(workflow.begin(changing: false), "Recovery is an explicit read-only refresh")
    try await rejectsAsync { _ = try await client.shares(local: "/cloud/file") }
    workflow.finish()
    try expect(!workflow.canChange && methods.filter { $0 == "POST" }.count == 1,
      "Repeated refresh failures never replay the saved mutation")
    failRefresh = false
    try expect(workflow.begin(changing: false), "Refresh stays available")
    _ = try await client.shares(local: "/cloud/file")
    workflow.refreshed()
    workflow.finish()
    try expect(workflow.canChange && methods.filter { $0 == "POST" }.count == 1,
      "A successful read unlocks new changes without replaying the old create")

    try expect(workflow.begin(changing: true), "A later explicit mutation starts")
    workflow.finish() // Simulates a lost response before confirmation.
    try expect(!workflow.changeConfirmed && !workflow.canChange && workflow.canRefresh,
      "An uncertain mutation requires read-only recovery before another write")
  }
  static func locking() async throws {
    var methods: [String] = []
    var supports = true
    var owner = "oliver"
    var type = "0"
    let transport = mocked { request in
      methods.append(request.httpMethod!)
      if request.url!.path.hasSuffix("/capabilities") {
        return (200, ocs(["capabilities": ["files": supports ? ["locking": "1.0"] : [:]]]))
      }
      if request.httpMethod == "PROPFIND" {
        return (
          207,
          dav(
            extra:
              "<nc:lock>1</nc:lock><nc:lock-owner>\(owner)</nc:lock-owner><nc:lock-owner-type>\(type)</nc:lock-owner-type>"
          )
        )
      }
      try expect(
        request.url!.path == "/nextcloud/ocs/v2.php/apps/files_lock/lock/42", "Lock endpoint")
      return (200, ocs())
    }
    let client = try NCClient(account: account(), password: "fixture", transport: transport)
    try await client.setLock(local: "/cloud/file", locked: true)
    try await client.setLock(local: "/cloud/file", locked: false)
    try expect(
      methods == ["GET", "PROPFIND", "PUT", "GET", "PROPFIND", "DELETE"],
      "Fresh capabilities before each lock action")
    methods = []
    supports = false
    try await rejectsAsync { try await client.setLock(local: "/cloud/file", locked: true) }
    try expect(methods == ["GET"], "Unsupported locking never mutates")
    supports = true
    for (user, lockType) in [
      ("other-user", "0"), ("oliver", "1"), ("oliver", "2"), ("", "0"), ("oliver", ""),
    ] {
      methods = []
      owner = user
      type = lockType
      try await rejectsAsync { try await client.setLock(local: "/cloud/file", locked: false) }
      try expect(!methods.contains("DELETE"), "Never override foreign, app or unknown lock")
    }
  }
  static func persistence() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      "schnauz-nextcloud-tests-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let file = root.appendingPathComponent("nextcloud.json")
    try NCStore.update(file) { $0.accounts.append(account()) }
    try expect(try NCStore.load(file).accounts.count == 1, "Config roundtrip")
    let permissions =
      try FileManager.default.attributesOfItem(atPath: file.path)[.posixPermissions] as! NSNumber
    try expect(permissions.intValue == 0o600, "Private config file")
    let saved = try String(contentsOf: file, encoding: .utf8)
    try expect(!saved.lowercased().contains("password"), "No credential field in config")
    let corrupt = Data("{invalid".utf8)
    try corrupt.write(to: file)
    do {
      try NCStore.update(file) { $0.accounts = [] }
      throw TestFailure.failed("Overwrote invalid settings")
    } catch is DecodingError {}
    try expect(try Data(contentsOf: file) == corrupt, "Corrupt settings preserved")
    try FileManager.default.removeItem(at: file)
    let group = DispatchGroup()
    let failures = NSLock()
    var errors = 0
    for index in 0..<12 {
      group.enter()
      DispatchQueue.global().async {
        defer { group.leave() }
        do {
          try NCStore.update(file) { $0.accounts.append(account(root: "/cloud-\(index)")) }
        } catch {
          failures.lock()
          errors += 1
          failures.unlock()
        }
      }
    }
    group.wait()
    try expect(
      errors == 0 && (try NCStore.load(file).accounts.count) == 12,
      "Concurrent updates preserve accounts")
  }
  static func main() async {
    do {
      try paths()
      print("PASS Nextcloud path and origin boundaries")
      try await login()
      print("PASS Nextcloud browser login, pending poll and cancellation")
      try await metadata()
      print("PASS Nextcloud DAV metadata and internal links")
      try await sharing()
      print("PASS Nextcloud explicit sharing and safe API failures")
      try await shareWorkflow()
      print("PASS Nextcloud confirmed-write/failed-refresh recovery, pending guards and read-only retry without duplicate shares")
      try await locking()
      print("PASS Nextcloud server capabilities and lock ownership")
      try persistence()
      print("PASS Nextcloud atomic settings and concurrent writers")
    } catch {
      fputs("Nextcloud test failed: \(error)\n", stderr)
      exit(1)
    }
  }
}
