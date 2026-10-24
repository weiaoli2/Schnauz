// Real hidden AppKit sharing windows with intercepted HTTPS. Never show/activate
// a window, access Keychain, invoke a File Provider domain, or contact a server.
import AppKit
import Foundation

enum ShareUITestFailure: Error, CustomStringConvertible {
  case failed(String)
  var description: String { switch self { case .failed(let message): return message } }
}
func uiExpect(_ condition: @autoclosure () -> Bool, _ message: String) throws {
  if !condition() { throw ShareUITestFailure.failed(message) }
}

final class ShareUIFixture: @unchecked Sendable {
  private let lock = NSLock()
  private var failedList = false
  private var methods: [String] = []
  private var shares: [[String: Any]] = [
    ["id": "1", "share_type": 3, "permissions": 1, "url": "https://fixture.invalid/s/old"]
  ]
  var failList: Bool {
    get { lock.lock(); defer { lock.unlock() }; return failedList }
    set { lock.lock(); defer { lock.unlock() }; failedList = newValue }
  }
  var requests: [String] { lock.lock(); defer { lock.unlock() }; return methods }
  func response(_ request: URLRequest) throws -> (Int, Data) {
    lock.lock(); defer { lock.unlock() }
    try uiExpect(request.url?.host == "fixture.invalid", "Only fixture requests are permitted")
    let method = request.httpMethod ?? ""
    methods.append(method)
    if method == "PROPFIND" {
      return (207, Data("""
        <d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns"><d:response><d:propstat><d:prop><oc:fileid>42</oc:fileid></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>
        """.utf8))
    }
    if method == "GET" {
      if failedList { return (503, Data()) }
      return (200, try ocs(shares))
    }
    try uiExpect(method == "POST", "No unexpected sharing operation")
    shares.append(["id": "2", "share_type": 3, "permissions": 1, "url": "https://fixture.invalid/s/new"])
    return (200, try ocs(["id": "2"]))
  }
  private func ocs(_ value: Any) throws -> Data {
    try JSONSerialization.data(withJSONObject: ["ocs": ["meta": ["statuscode": 200], "data": value]])
  }
}

final class ShareUIProtocol: URLProtocol {
  // Installed once before the asynchronous fixture requests start.
  static var fixture: ShareUIFixture!
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    do {
      let (status, data) = try Self.fixture.response(request)
      client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status,
        httpVersion: "HTTP/1.1", headerFields: nil)!, cacheStoragePolicy: .notAllowed)
      client?.urlProtocol(self, didLoad: data)
      client?.urlProtocolDidFinishLoading(self)
    } catch { client?.urlProtocol(self, didFailWithError: error) }
  }
  override func stopLoading() {}
}

@main struct ShareUITests {
  @MainActor static func wait(_ form: SharesWindow) async throws {
    guard let work = form.work else { throw ShareUITestFailure.failed("Expected a real sharing task") }
    await work.value
    try uiExpect(!form.workflow.pending, "Sharing task must clear its busy state")
    try uiExpect(!form.window.isVisible && !form.window.isKeyWindow, "Test windows must remain hidden")
    try uiExpect(NSApplication.shared.modalWindow == nil, "No native modal session may be started")
  }
  @MainActor static func checkGeometry(_ form: SharesWindow, dark: Bool) throws {
    let content = form.window.contentView!
    form.window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
    form.window.setFrame(NSRect(origin: NSPoint(x: -20000, y: -20000), size: form.window.minSize), display: false)
    content.layoutSubtreeIfNeeded()
    try uiExpect(form.window.frame.size == form.window.minSize, "Geometry is checked at the actual minimum window size")
    let bounds = content.bounds.insetBy(dx: -0.5, dy: -0.5)
    var previous: NSRect?
    for button in form.buttons {
      let rect = button.convert(button.bounds, to: content)
      // AppKit button frames include shadow/alignment padding. NSStackView
      // spaces their alignment rects; overlapping shadow frames are normal.
      let alignment = button.superview!.convert(button.alignmentRect(forFrame: button.frame), to: content)
      try uiExpect(rect.width >= 45 && rect.height >= 20, "\(dark ? "Dark" : "Light") button has unusable bounds: \(button.title) \(rect)")
      try uiExpect(bounds.contains(rect), "\(dark ? "Dark" : "Light") button leaves minimum-size window: \(button.title) \(rect), content \(bounds)")
      if let previous { try uiExpect(previous.maxX <= alignment.minX + 0.5, "Sharing button alignment rects overlap: previous \(previous), \(button.title) \(alignment)") }
      previous = alignment
    }
    guard let refresh = form.refreshButton else { throw ShareUITestFailure.failed("Native Refresh button is missing") }
    let refreshRect = refresh.convert(refresh.bounds, to: content)
    try uiExpect(refreshRect.width >= 45 && refreshRect.height >= 20 && bounds.contains(refreshRect),
      "Refresh must remain usable at the minimum window size: \(refreshRect)")
    try uiExpect(form.table.enclosingScrollView!.frame.height >= 149, "The share list retains a readable viewport")
  }
  @MainActor static func main() async {
    DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + 20) {
      fputs("Nextcloud sharing UI check timed out\n", stderr)
      exit(1)
    }
    let app = NSApplication.shared
    app.setActivationPolicy(.prohibited)
    do {
      let fixture = ShareUIFixture()
      ShareUIProtocol.fixture = fixture
      let configuration = URLSessionConfiguration.ephemeral
      configuration.protocolClasses = [ShareUIProtocol.self]
      let account = NCAccount(id: UUID().uuidString, root: "/fixture/cloud",
        server: "https://fixture.invalid", login: "fixture", userID: "fixture", remoteRoot: "/", locking: false)
      let client = try NCClient(account: account, password: "fixture", transport: NCTransport(configuration: configuration))
      let form = SharesWindow(client: client, path: "/fixture/cloud/track.mp3")
      try uiExpect(fixture.requests.isEmpty, "Constructing a hidden window makes no requests")
      for index in [0,1,2,4] { try uiExpect(!form.buttons[index].isEnabled, "Initial mutation actions stay disabled") }
      try uiExpect(form.refreshButton!.isEnabled, "Initial Refresh is available")
      form.refresh()
      try uiExpect(form.workflow.pending && form.buttons.allSatisfy { !$0.isEnabled }, "Controls disable while the real read task is running")
      try await wait(form)
      try uiExpect(fixture.requests == ["PROPFIND", "GET"], "Opening/loading shares sends no writes")
      try uiExpect(form.rows.map(\.id) == ["1"] && form.table.numberOfRows == 1 && form.workflow.canChange, "Successful initial load populates the actual native table")

      fixture.failList = true
      form.perform { try await client.createShare(local: "/fixture/cloud/track.mp3", type: 3) }
      try await wait(form)
      try uiExpect(form.rows.map(\.id) == ["1"] && form.table.numberOfRows == 1, "Confirmed write with failed refresh retains the old native table rows")
      try uiExpect(form.workflow.changeConfirmed && !form.workflow.canChange, "Confirmed save requires read-only recovery")
      try uiExpect(form.label.stringValue.contains("Change saved"), "A failed list refresh must not report a failed write")
      try uiExpect(schnauzNextcloudUITestErrors.count == 1 && schnauzNextcloudUITestErrors[0].contains("saved"), "Error is captured without showing a modal")
      for index in [0,1,2,4] { try uiExpect(!form.buttons[index].isEnabled, "Further mutations must stay disabled") }
      var replayed = false
      form.perform { replayed = true; try await client.createShare(local: "/fixture/cloud/track.mp3", type: 3) }
      await Task.yield()
      try uiExpect(!replayed && fixture.requests.filter { $0 == "POST" }.count == 1, "Recovery never resubmits the mutation")

      fixture.failList = false
      form.refreshButton!.performClick(nil)
      try await wait(form)
      try uiExpect(form.rows.map(\.id) == ["1","2"] && form.table.numberOfRows == 2 && form.workflow.canChange, "Native Refresh button recovers the updated native table")
      try uiExpect(form.buttons.allSatisfy(\.isEnabled), "Mutation controls return only after a successful read")
      try uiExpect(fixture.requests.filter { $0 == "POST" }.count == 1, "Native Refresh sends no duplicate create")
      for dark in [false,true] { try checkGeometry(form, dark: dark) }
      try uiExpect(!form.window.isVisible && !form.window.isKeyWindow && app.modalWindow == nil,
        "Hidden sharing regressions must never present UI")
      print("PASS real hidden Nextcloud sharing window: light/dark minimum geometry, initial/pending controls, read-only opening, confirmed-write/failed-refresh recovery, retained rows, no duplicate write and native Refresh action; fixture HTTPS only")
    } catch {
      fputs("Nextcloud sharing UI check failed: \(error)\n", stderr)
      exit(1)
    }
  }
}
