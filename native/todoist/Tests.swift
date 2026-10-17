// Entirely offline fixtures: no real Todoist requests or Keychain access.
import Foundation

enum TDFixtureFailure: Error { case failed(String) }
func check(_ value: @autoclosure () throws -> Bool, _ message: String) throws {
  if try !value() { throw TDFixtureFailure.failed(message) }
}
func failure(_ action: () async throws -> Void) async throws -> TDProblem {
  do { try await action() } catch let error as TDProblem { return error }
  throw TDFixtureFailure.failed("Expected a safe Todoist error")
}
final class FixtureProtocol: URLProtocol {
  static var handler: ((URLRequest) throws -> (Int, Data, [String: String]))!
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    do {
      let (status, data, headers) = try Self.handler(request)
      client?.urlProtocol(
        self,
        didReceive: HTTPURLResponse(
          url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!,
        cacheStoragePolicy: .notAllowed)
      client?.urlProtocol(self, didLoad: data)
      client?.urlProtocolDidFinishLoading(self)
    } catch { client?.urlProtocol(self, didFailWithError: error) }
  }
  override func stopLoading() {}
}
func encode(_ value: Any) -> Data { try! JSONSerialization.data(withJSONObject: value) }
func mocked(_ handler: @escaping (URLRequest) throws -> (Int, Data, [String: String])) throws
  -> TDClient
{
  FixtureProtocol.handler = { request in
    try check(
      request.url?.host == "api.todoist.com" && request.url?.scheme == "https",
      "Token bound to HTTPS Todoist host")
    try check(
      request.value(forHTTPHeaderField: "Authorization") == "Bearer fixture-token",
      "Bearer credential header")
    try check(!(request.url!.absoluteString.contains("fixture-token")), "No token in URL")
    return try handler(request)
  }
  let config = URLSessionConfiguration.ephemeral
  config.protocolClasses = [FixtureProtocol.self]
  return try TDClient(token: "fixture-token", configuration: config)
}
func payload(_ request: URLRequest) throws -> [String: Any] {
  var data = request.httpBody ?? Data()
  if data.isEmpty, let stream = request.httpBodyStream {
    stream.open()
    defer { stream.close() }
    var bytes = [UInt8](repeating: 0, count: 1024)
    while stream.hasBytesAvailable {
      let count = stream.read(&bytes, maxLength: bytes.count)
      if count <= 0 { break }
      data.append(contentsOf: bytes.prefix(count))
    }
  }
  return try JSONSerialization.jsonObject(with: data) as? [String: Any] ?? [:]
}
func project(_ name: String = "Work", id: String = "project-A") -> [String: Any] {
  ["id": id, "name": name, "parent_id": NSNull()]
}
func task(_ id: String = "task-A", project: String = "project-A", extra: [String: Any] = [:])
  -> [String: Any]
{
  ["id": id, "project_id": project, "content": "Test task", "priority": 1].merging(extra) {
    _, new in new
  }
}
func page(_ rows: [[String: Any]], cursor: String? = nil) -> Data {
  encode(["results": rows, "next_cursor": cursor as Any? ?? NSNull()])
}

@main struct TodoistTests {
  static func matching() async throws {
    var writes = 0
    let client = try mocked { request in
      if request.httpMethod == "POST" {
        writes += 1
        return (200, encode(project()), [:])
      }
      let query = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems ?? []
      try check(query.contains(URLQueryItem(name: "limit", value: "200")), "Valid pagination size")
      if query.contains(URLQueryItem(name: "cursor", value: "opaque+ &cursor")) {
        return (200, page([project("work")]), [:])
      }
      return (200, page([project("Other", id: "project-B")], cursor: "opaque+ &cursor"), [:])
    }
    let found = try await client.resolveProject(
      name: " Work ", allowCreate: true, requestID: "stable-project-request")
    try check(
      found.count == 1 && found[0].name == "work" && writes == 0,
      "Existing matching project is linked, never recreated")
    let duplicates = try mocked { _ in (200, page([project(), project(id: "second-Work")]), [:]) }
    let matches = try await duplicates.resolveProject(
      name: "Work", allowCreate: true, requestID: "stable")
    try check(matches.count == 2, "Duplicate names remain choices, no silent selection")
    var posted = false
    let missing = try mocked { request in
      if request.httpMethod == "GET" { return (200, page([]), [:]) }
      posted = true
      let body = try payload(request)
      try check(body["name"] as? String == "New Project", "New project keeps Schnauz name")
      try check(
        request.value(forHTTPHeaderField: "X-Request-Id") == "stable-project-request",
        "Persistent idempotency key")
      return (200, encode(project("New Project")), [:])
    }
    let absent = try await missing.resolveProject(
      name: "New Project", allowCreate: false, requestID: nil)
    try check(
      absent.isEmpty && !posted, "Existing unlinked project lookup never creates implicitly")
    let created = try await missing.resolveProject(
      name: "New Project", allowCreate: true, requestID: "stable-project-request")
    try check(
      created[0].name == "New Project" && posted, "Explicitly requested new project creates once")
    let offline = try mocked { request in
      try check(request.httpMethod == "GET", "No creation after failed project lookup")
      return (503, Data(), [:])
    }
    _ = try await failure {
      _ = try await offline.resolveProject(name: "Work", allowCreate: true, requestID: "stable")
    }
    let malformed = try mocked { request in
      try check(request.httpMethod == "GET", "Invalid pages cannot trigger project creation")
      return (200, encode(["unexpected": []]), [:])
    }
    _ = try await failure {
      _ = try await malformed.resolveProject(name: "Work", allowCreate: true, requestID: "stable")
    }
  }
  static func paginationAndSnapshot() async throws {
    let client = try mocked { request in
      let url = request.url!
      let parts = URLComponents(url: url, resolvingAgainstBaseURL: false)!.queryItems ?? []
      if url.path.hasSuffix("/projects/project-A") { return (200, encode(project()), [:]) }
      try check(
        parts.contains(URLQueryItem(name: "project_id", value: "project-A")),
        "Every page remains scoped to the captured project")
      if url.path.hasSuffix("/sections") {
        return (
          200,
          page([
            ["id": "section-A", "project_id": "project-A", "name": "Section", "section_order": 1],
            ["id": "wrong", "project_id": "other", "name": "Other"],
          ]), [:]
        )
      }
      if parts.contains(URLQueryItem(name: "cursor", value: "next-page")) {
        return (
          200,
          page([
            task(
              "child",
              extra: ["parent_id": "task-A", "due": ["date": "2026-12-31", "is_recurring": true]]),
            task(), task("deleted", extra: ["is_deleted": true]),
          ]), [:]
        )
      }
      return (
        200,
        page(
          [task(), task("wrong", project: "other"), task("done", extra: ["checked": true])],
          cursor: "next-page"), [:]
      )
    }
    let snapshot = try await client.snapshot(projectID: "project-A")
    try check(
      snapshot.tasks.map(\.id) == ["task-A", "child"],
      "All pages deduplicated; foreign/completed/deleted tasks excluded")
    try check(snapshot.sections.map(\.id) == ["section-A"], "Sections scoped to same project")
    try check(
      snapshot.tasks[1].parent_id == "task-A" && snapshot.tasks[1].due?.is_recurring == true,
      "Hierarchy and recurrence preserved")
    let repeated = try mocked { _ in (200, page([], cursor: "same"), [:]) }
    _ = try await failure { _ = try await repeated.projects() }
  }
  static func writes() async throws {
    var posts = 0
    let client = try mocked { request in
      if request.httpMethod == "GET" { return (200, encode(task()), [:]) }
      posts += 1
      try check(
        request.value(forHTTPHeaderField: "X-Request-Id") == "stable-task-request",
        "Write idempotency header")
      if request.url!.path.hasSuffix("/close") {
        try check(request.url!.path == "/api/v1/tasks/task-A/close", "Only captured task completed")
        return (204, Data(), [:])
      }
      let data = try payload(request)
      try check(data["project_id"] as? String == "project-A", "Adding never falls back to Inbox")
      try check(
        data["content"] as? String == "Buy café + & milk"
          && data["due_string"] as? String == "tomorrow", "Task content encoded losslessly")
      return (200, encode(task()), [:])
    }
    _ = try await client.add(
      projectID: "project-A", content: "Buy café + & milk", due: "tomorrow", section: nil,
      requestID: "stable-task-request")
    try await client.complete(
      projectID: "project-A", taskID: "task-A", requestID: "stable-task-request")
    try check(posts == 2, "Only explicit add and complete perform writes")
    let moved = try mocked { request in
      try check(request.httpMethod == "GET", "Moved tasks are not completed in another project")
      return (200, encode(task(project: "other")), [:])
    }
    _ = try await failure {
      try await moved.complete(projectID: "project-A", taskID: "task-A", requestID: "stable")
    }
    _ = try await failure {
      _ = try await moved.add(
        projectID: "", content: "Task", due: "", section: nil, requestID: "stable")
    }
    let changed = try mocked { request in
      try check(request.httpMethod == "GET", "A changed recurring occurrence is not closed again")
      return (200, encode(task(extra: ["updated_at": "new-occurrence"])), [:])
    }
    _ = try await failure {
      try await changed.complete(
        projectID: "project-A", taskID: "task-A", requestID: "same-attempt",
        expectedUpdated: "old-occurrence")
    }
  }
  static func safeFailures() async throws {
    let limited = try mocked { _ in (429, encode(["error_extra": ["retry_after": 45]]), [:]) }
    let error = try await failure { _ = try await limited.projects() }
    try check(error.retryAfter == 45, "Server backoff preserved")
    for code in [401, 403, 404, 302, 500] {
      let failed = try mocked { _ in (code, encode(["error": "private payload fixture-token"]), [:])
      }
      let error = try await failure { _ = try await failed.projects() }
      try check(
        !error.text.contains("fixture-token") && !error.text.contains("private payload"),
        "Server payload and token stay out of errors")
    }
    let url = TDClient.endpoint(
      ["tasks", "a?b#c"], query: [URLQueryItem(name: "cursor", value: "+ &")])
    try check(
      url.path.hasSuffix("/a?b#c") && url.host == "api.todoist.com",
      "Opaque IDs cannot change endpoint origin")
    let query = URLComponents(url: url, resolvingAgainstBaseURL: false)!.percentEncodedQuery!
    let value = String(query.split(separator: "=", maxSplits: 1)[1])
      .replacingOccurrences(of: "+", with: " ").removingPercentEncoding
    try check(value == "+ &", "Opaque cursors survive server-side form query parsing")
  }
  static func storage() throws {
    let fm = Foundation.FileManager()
    let root = fm.temporaryDirectory.appendingPathComponent(
      "schnauz-todoist-test-" + UUID().uuidString)
    defer { try? fm.removeItem(at: root) }
    let file = root.appendingPathComponent("todoist.json")
    let account = TDAccount(id: UUID().uuidString, userID: "test-user", name: "Test User")
    try TDStore.save(TDConfig(account: account), to: file)
    try check(try TDStore.load(file).account?.userID == "test-user", "Account metadata roundtrip")
    let saved = try String(contentsOf: file, encoding: .utf8)
    try check(!saved.contains("token"), "No token in settings")
    let bad = Data("invalid".utf8)
    try bad.write(to: file)
    do {
      try TDStore.save(TDConfig(), to: file)
      throw TDFixtureFailure.failed("Corrupt config overwritten")
    } catch is DecodingError {}
    try check(try Data(contentsOf: file) == bad, "Invalid settings preserved")
  }
  static func main() async {
    do {
      try await matching()
      print("PASS Todoist name matching, existing projects, duplicate names and explicit creation")
      try await paginationAndSnapshot()
      print("PASS Todoist pagination, project scope and task hierarchy")
      try await writes()
      print("PASS Todoist captured destinations, idempotent writes and moved-task protection")
      try await safeFailures()
      print("PASS Todoist safe errors, rate limits and endpoint boundaries")
      try storage()
      print("PASS Todoist metadata and corrupt-config preservation")
    } catch {
      fputs("Todoist test failed: \(error)\n", stderr)
      exit(1)
    }
  }
}
