import Darwin
// Schnauz's independent Todoist API v1 client. Credentials never leave Keychain
// except in memory for HTTPS requests to the fixed Todoist API host.
import Foundation
import Security
import LocalAuthentication

struct TDProblem: LocalizedError {
  let text: String
  var retryAfter: Int? = nil
  var errorDescription: String? { text }
}
struct TDAccount: Codable {
  let id: String
  let userID: String
  let name: String
}
struct TDConfig: Codable {
  var version = 1
  var account: TDAccount? = nil
}
struct TDProject: Codable {
  let id: String
  let name: String
  let parent_id: String?
  var display_path: String? = nil
}
struct TDSection: Codable {
  let id: String
  let project_id: String
  let name: String
  let section_order: Int?
  let order_key: String?
}
struct TDDue: Codable {
  let date: String?
  let datetime: String?
  let string: String?
  let is_recurring: Bool?
}
struct TDTask: Codable {
  let id: String
  let project_id: String
  let content: String
  let description: String?
  let section_id: String?
  let parent_id: String?
  let priority: Int?
  let child_order: Int?
  let order_key: String?
  let checked: Bool?
  let updated_at: String?
  let is_deleted: Bool?
  let due: TDDue?
}
struct TDSnapshot: Codable {
  let project: TDProject
  let tasks: [TDTask]
  let sections: [TDSection]
}
struct TDTaskOptions {
  var description = ""
  var deadline: String? = nil
  var priority = 1
  var labels: [String] = []
}
struct TDLocation {
  let name: String
  let latitude: String
  let longitude: String
  let trigger: String
  let radius: Int
}
private struct TDPage<T: Decodable>: Decodable {
  let results: [T]
  let next_cursor: String?
  enum CodingKeys: String, CodingKey { case results, next_cursor }
  init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)
    guard values.contains(.next_cursor) else {
      throw DecodingError.keyNotFound(
        CodingKeys.next_cursor,
        .init(codingPath: decoder.codingPath, debugDescription: "Missing pagination terminator"))
    }
    results = try values.decode([T].self, forKey: .results)
    next_cursor = try values.decodeIfPresent(String.self, forKey: .next_cursor)
  }
}

final class TDClient: NSObject, URLSessionTaskDelegate {
  private let token: String
  private let session: URLSession
  init(token: String, configuration: URLSessionConfiguration = .ephemeral) throws {
    let trimmed = token.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty, trimmed.utf8.count < 4096,
      !trimmed.contains(where: { $0.isWhitespace || $0.isNewline })
    else {
      throw TDProblem(text: "Enter your Todoist API token.")
    }
    self.token = trimmed
    configuration.httpCookieStorage = nil
    configuration.urlCredentialStorage = nil
    configuration.timeoutIntervalForRequest = 25
    configuration.timeoutIntervalForResource = 40
    session = URLSession(configuration: configuration)
    super.init()
  }
  static func identifier(_ id: String) throws -> String {
    guard !id.isEmpty, id.count < 256, !id.contains(where: { $0.isWhitespace || $0.isNewline }),
      !id.contains("/"), !id.contains("\0"), id != ".", id != ".."
    else {
      throw TDProblem(text: "This Todoist identifier is invalid. Link the project again.")
    }
    return id
  }
  static func endpoint(_ parts: [String], query: [URLQueryItem] = []) -> URL {
    let unreserved = CharacterSet(
      charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~")
    var url = URLComponents(string: "https://api.todoist.com")!
    url.percentEncodedPath =
      "/api/v1/"
      + parts.map { $0.addingPercentEncoding(withAllowedCharacters: unreserved)! }.joined(
        separator: "/")
    url.queryItems = query.isEmpty ? nil : query
    // A literal '+' in an opaque cursor must survive form-style query parsers.
    if let query = url.percentEncodedQuery {
      url.percentEncodedQuery = query.replacingOccurrences(of: "+", with: "%2B")
    }
    return url.url!
  }
  func urlSession(
    _ session: URLSession, task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
    completionHandler: @escaping (URLRequest?) -> Void
  ) { completionHandler(nil) }
  func request(
    _ parts: [String], method: String = "GET", query: [URLQueryItem] = [],
    json: [String: Any]? = nil, requestID: String? = nil
  ) async throws -> Data {
    try Task.checkCancellation()
    var request = URLRequest(url: Self.endpoint(parts, query: query))
    request.httpMethod = method
    request.setValue("Bearer " + token, forHTTPHeaderField: "Authorization")
    request.setValue("Schnauz", forHTTPHeaderField: "User-Agent")
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    if let json {
      request.httpBody = try JSONSerialization.data(withJSONObject: json)
      request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    }
    if let requestID { request.setValue(requestID, forHTTPHeaderField: "X-Request-Id") }
    do {
      let (data, response) = try await session.data(for: request, delegate: self)
      guard let http = response as? HTTPURLResponse, data.count <= 8 * 1024 * 1024 else {
        throw TDProblem(text: "Todoist returned an invalid or oversized response.")
      }
      guard (200..<300).contains(http.statusCode) else {
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        let extra = json?["error_extra"] as? [String: Any]
        let delay =
          Int(http.value(forHTTPHeaderField: "Retry-After") ?? "")
          ?? (extra?["retry_after"] as? Int)
        let message: String
        switch http.statusCode {
        case 401:
          message = "Todoist access has expired or was revoked. Reconnect Todoist in Settings."
        case 403:
          message = ["reminders", "location_reminders"].contains(parts.first ?? "")
            ? "Your Todoist account or plan does not permit this reminder."
            : "Todoist does not permit this action. Check your access to this project."
        case 404:
          message =
            "This Todoist project or task is no longer available. Refresh or link another project."
        case 429: message = "Todoist is limiting requests. Please wait before refreshing."
        case 300..<400:
          message = "Todoist redirected the request. Schnauz did not forward your token."
        case 400:
          message = "Todoist rejected the task details. Check the name and optional due date."
        default: message = "Todoist could not finish this request (HTTP \(http.statusCode))."
        }
        throw TDProblem(
          text: message,
          retryAfter: delay.map { min(3600, max(5, $0)) } ?? (http.statusCode == 429 ? 60 : nil))
      }
      return data
    } catch is CancellationError { throw CancellationError() } catch let problem as TDProblem {
      throw problem
    } catch let error as URLError {
      if error.code == .cancelled { throw CancellationError() }
      throw TDProblem(
        text: method == "GET"
          ? "Cannot reach Todoist. Cached tasks may be out of date."
          : "Todoist did not confirm the change. Check Todoist before trying a different task; retrying the same request is safe.",
        retryAfter: 15)
    }
  }
  func decoded<T: Decodable>(_ type: T.Type, parts: [String]) async throws -> T {
    do { return try JSONDecoder().decode(type, from: await request(parts)) } catch is DecodingError
    {
      throw TDProblem(
        text: "Todoist returned an unexpected response. Your previous tasks have been kept.")
    }
  }
  func all<T: Decodable>(_ type: T.Type, parts: [String], query: [URLQueryItem] = []) async throws
    -> [T]
  {
    var cursor: String? = nil
    var visited = Set<String>()
    var results: [T] = []
    for _ in 0..<100 {
      var fields = query + [URLQueryItem(name: "limit", value: "200")]
      if let cursor { fields.append(URLQueryItem(name: "cursor", value: cursor)) }
      let data = try await request(parts, query: fields)
      let page: TDPage<T>
      do { page = try JSONDecoder().decode(TDPage<T>.self, from: data) } catch {
        throw TDProblem(text: "Todoist returned an unexpected page. Cached tasks have been kept.")
      }
      results += page.results
      guard let next = page.next_cursor, !next.isEmpty else { return results }
      guard visited.insert(next).inserted else {
        throw TDProblem(text: "Todoist repeated a page cursor. Refresh again later.")
      }
      cursor = next
    }
    throw TDProblem(text: "This Todoist list is too large to load in Schnauz. Open it in Todoist.")
  }
  func identity() async throws -> (String, String) {
    let data = try await request(["user"])
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
      let id = object["id"] as? String ?? object["sub"] as? String
        ?? (object["id"] as? NSNumber)?.stringValue
    else { throw TDProblem(text: "Could not identify this Todoist account.") }
    return (
      try Self.identifier(id),
      object["full_name"] as? String ?? object["name"] as? String ?? "Todoist"
    )
  }
  func projects() async throws -> [TDProject] {
    let rows = try await all(TDProject.self, parts: ["projects"])
    var seen = Set<String>()
    let unique = rows.filter { seen.insert($0.id).inserted }
    let byID = Dictionary(uniqueKeysWithValues: unique.map { ($0.id, $0) })
    return unique.map { row in
      var project = row
      var names = [row.name]
      var parent = row.parent_id
      var visited = Set([row.id])
      while let id = parent, visited.insert(id).inserted, let ancestor = byID[id], names.count < 32
      {
        names.insert(ancestor.name, at: 0)
        parent = ancestor.parent_id
      }
      project.display_path = names.joined(separator: " / ")
      return project
    }
  }
  func matchingProjects(name: String) async throws -> [TDProject] {
    let normalized = name.trimmingCharacters(in: .whitespacesAndNewlines)
      .precomposedStringWithCanonicalMapping
    return try await projects().filter {
      $0.name.trimmingCharacters(in: .whitespacesAndNewlines).precomposedStringWithCanonicalMapping
        .caseInsensitiveCompare(normalized) == .orderedSame
    }
  }
  func createProject(name: String, requestID: String) async throws -> TDProject {
    let text = name.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty, text.count <= 120 else {
      throw TDProblem(text: "Todoist project names must contain 1–120 characters.")
    }
    let data = try await request(
      ["projects"], method: "POST", json: ["name": text], requestID: requestID)
    return try JSONDecoder().decode(TDProject.self, from: data)
  }
  func resolveProject(name: String, allowCreate: Bool, requestID: String?) async throws
    -> [TDProject]
  {
    let matches = try await matchingProjects(name: name)
    if !matches.isEmpty || !allowCreate { return matches }
    guard let requestID, !requestID.isEmpty else {
      throw TDProblem(text: "Could not identify the project creation request.")
    }
    return [try await createProject(name: name, requestID: requestID)]
  }
  func snapshot(projectID: String) async throws -> TDSnapshot {
    let id = try Self.identifier(projectID)
    let project = try await decoded(TDProject.self, parts: ["projects", id])
    guard project.id == id else { throw TDProblem(text: "Todoist returned the wrong project.") }
    let filter = [URLQueryItem(name: "project_id", value: id)]
    async let fetchedTasks = all(TDTask.self, parts: ["tasks"], query: filter)
    async let fetchedSections = all(TDSection.self, parts: ["sections"], query: filter)
    let (tasks, sections) = try await (fetchedTasks, fetchedSections)
    var taskIDs = Set<String>()
    var sectionIDs = Set<String>()
    return TDSnapshot(
      project: project,
      tasks: tasks.filter {
        $0.project_id == id && $0.checked != true && $0.is_deleted != true
          && taskIDs.insert($0.id).inserted
      },
      sections: sections.filter { $0.project_id == id && sectionIDs.insert($0.id).inserted })
  }
  func add(projectID: String, content: String, due: String, section: String?, requestID: String,
    options: TDTaskOptions = TDTaskOptions())
    async throws -> TDTask
  {
    let project = try Self.identifier(projectID)
    let text = content.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty, text.count <= 500 else {
      throw TDProblem(text: "Enter a task name of up to 500 characters.")
    }
    guard options.description.count <= 16_000, (1...4).contains(options.priority), options.labels.count <= 50,
      options.labels.allSatisfy({ !$0.isEmpty && $0.count <= 60 }) else {
      throw TDProblem(text: "Check the description, priority and labels.")
    }
    if let deadline = options.deadline {
      let formatter = DateFormatter(); formatter.locale = Locale(identifier: "en_US_POSIX")
      formatter.calendar = Calendar(identifier: .gregorian); formatter.dateFormat = "yyyy-MM-dd"
      formatter.isLenient = false
      guard let date = formatter.date(from: deadline), formatter.string(from: date) == deadline else {
        throw TDProblem(text: "Choose a valid deadline date.")
      }
    }
    var fields: [String: Any] = ["content": text, "project_id": project, "priority": options.priority]
    if !options.description.isEmpty { fields["description"] = options.description }
    if !options.labels.isEmpty { fields["labels"] = Array(Set(options.labels)).sorted() }
    if let deadline = options.deadline { fields["deadline_date"] = deadline }
    if !due.isEmpty { fields["due_string"] = due }
    if let section, !section.isEmpty { fields["section_id"] = try Self.identifier(section) }
    let data = try await request(["tasks"], method: "POST", json: fields, requestID: requestID)
    let task = try JSONDecoder().decode(TDTask.self, from: data)
    guard task.project_id == project else {
      throw TDProblem(
        text: "Todoist did not confirm the destination project. Check the task in Todoist.")
    }
    return task
  }
  func project(_ projectID: String) async throws -> TDProject {
    let id = try Self.identifier(projectID)
    let project = try await decoded(TDProject.self, parts: ["projects", id])
    guard project.id == id else { throw TDProblem(text: "Todoist returned the wrong project.") }
    return project
  }
  func reminder(taskID: String, date: Date, requestID: String) async throws {
    let id = try Self.identifier(taskID)
    guard date > Date() else { throw TDProblem(text: "Choose a reminder time in the future.") }
    let when = ISO8601DateFormatter().string(from: date)
    let data = try await request(["reminders"], method: "POST", json: [
      "task_id": id, "reminder_type": "absolute", "due": ["date": when], "service": "push"
    ], requestID: requestID)
    guard let value = try JSONSerialization.jsonObject(with: data) as? [String: Any],
      value["item_id"] as? String == id else {
      throw TDProblem(text: "Todoist did not confirm the reminder for this task.")
    }
  }
  func locationReminder(taskID: String, projectID: String, location: TDLocation, requestID: String) async throws {
    let id = try Self.identifier(taskID)
    guard !location.name.isEmpty, location.name.count <= 255,
      let lat = Double(location.latitude), (-90...90).contains(lat),
      let lon = Double(location.longitude), (-180...180).contains(lon),
      ["on_enter", "on_leave"].contains(location.trigger), (1...255).contains(location.radius) else {
      throw TDProblem(text: "Choose a valid reminder location.")
    }
    let data = try await request(["location_reminders"], method: "POST", json: [
      "task_id": id, "name": location.name, "loc_lat": location.latitude,
      "loc_long": location.longitude, "loc_trigger": location.trigger, "radius": location.radius
    ], requestID: requestID)
    guard let value = try JSONSerialization.jsonObject(with: data) as? [String: Any],
      value["item_id"] as? String == id, value["project_id"] as? String == projectID else {
      throw TDProblem(text: "Todoist did not confirm the location reminder for this project.")
    }
  }
  func complete(
    projectID: String, taskID: String, requestID: String, expectedUpdated: String? = nil
  ) async throws {
    let project = try Self.identifier(projectID)
    let taskID = try Self.identifier(taskID)
    let task = try await decoded(TDTask.self, parts: ["tasks", taskID])
    guard task.project_id == project else {
      throw TDProblem(
        text: "This task has moved to another Todoist project. Refresh before completing it.")
    }
    if let expectedUpdated, task.updated_at != expectedUpdated {
      throw TDProblem(
        text:
          "This task changed in Todoist. Refresh before completing it; a recurring task may already have advanced to its next date."
      )
    }
    _ = try await request(["tasks", taskID, "close"], method: "POST", requestID: requestID)
  }
}

enum TDStore {
  static func load(_ file: URL) throws -> TDConfig {
    if !Foundation.FileManager().fileExists(atPath: file.path) { return TDConfig() }
    let data = try Data(contentsOf: file)
    guard data.count <= 65536 else {
      throw TDProblem(text: "Todoist settings are invalid. The file has been preserved.")
    }
    let config = try JSONDecoder().decode(TDConfig.self, from: data)
    guard config.version == 1,
      config.account.map({ UUID(uuidString: $0.id) != nil && !$0.userID.isEmpty }) ?? true
    else { throw TDProblem(text: "Todoist settings are invalid. The file has been preserved.") }
    return config
  }
  static func save(_ config: TDConfig, to file: URL) throws {
    _ = try load(file)  // Never overwrite unreadable or incompatible settings.
    try Foundation.FileManager().createDirectory(
      at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    try JSONEncoder().encode(config).write(to: file, options: .atomic)
    try Foundation.FileManager().setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
  }
}
enum TDKeychain {
  static let service = "local.schnauz.todoist"
  static func store(_ token: String, account: TDAccount) throws {
    let payload = try JSONSerialization.data(withJSONObject: [
      "token": token, "userID": account.userID,
    ])
    let query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
      kSecAttrAccount as String: account.id,
    ]
    let status = SecItemAdd(
      query.merging([
        kSecValueData as String: payload,
        kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
      ]) { _, new in new } as CFDictionary, nil)
    let result =
      status == errSecDuplicateItem
      ? SecItemUpdate(query as CFDictionary, [kSecValueData as String: payload] as CFDictionary)
      : status
    guard result == errSecSuccess else {
      throw TDProblem(text: "Could not save Todoist access in macOS Keychain.")
    }
  }
  static func read(_ account: TDAccount, allowInteraction: Bool = true) throws -> String {
    var item: CFTypeRef?
    var query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
      kSecAttrAccount as String: account.id, kSecReturnData as String: true,
      kSecMatchLimit as String: kSecMatchLimitOne,
    ]
    if !allowInteraction {
      let context = LAContext(); context.interactionNotAllowed = true
      query[kSecUseAuthenticationContext as String] = context
    }
    guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
      let data = item as? Data,
      let value = try JSONSerialization.jsonObject(with: data) as? [String: String],
      value["userID"] == account.userID, let token = value["token"]
    else {
      throw TDProblem(text: "Todoist access is missing in Keychain. Reconnect Todoist in Settings.")
    }
    return token
  }
  static func remove(_ id: String) {
    SecItemDelete(
      [
        kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
        kSecAttrAccount as String: id,
      ] as CFDictionary)
  }
}
