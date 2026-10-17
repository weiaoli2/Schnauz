// Native UI and process bridge for Schnauz's own Nextcloud connection.
import AppKit
import Foundation

struct ToolRequest: Decodable {
  let operation: String
  let config: String
  let root: String?
  let path: String?
}
struct ToolResult: Encodable {
  let success: Bool
  let message: String
  var cancelled = false
}

@MainActor func finish(_ result: ToolResult) -> Never {
  if let data = try? JSONEncoder().encode(result) { FileHandle.standardOutput.write(data) }
  exit(result.success || result.cancelled ? 0 : 1)
}
@MainActor func errorAlert(_ error: Error) {
  let alert = NSAlert()
  alert.messageText = "Nextcloud"
  alert.informativeText =
    (error as? NCFailure)?.localizedDescription
    ?? "Nextcloud could not complete this action. Check the connection and try again."
  alert.addButton(withTitle: "OK")
  alert.runModal()
}
@MainActor func prompt(
  _ title: String, detail: String, fields: [(String, String, Bool)], confirm: String = "Continue"
) -> [String]? {
  let alert = NSAlert()
  alert.messageText = title
  alert.informativeText = detail
  alert.addButton(withTitle: confirm)
  alert.addButton(withTitle: "Cancel")
  let stack = NSStackView()
  stack.orientation = .vertical
  stack.alignment = .leading
  stack.spacing = 7
  var inputs: [NSTextField] = []
  for (name, value, secure) in fields {
    let label = NSTextField(labelWithString: name)
    label.font = .systemFont(ofSize: 11)
    let input = secure ? NSSecureTextField(string: value) : NSTextField(string: value)
    input.widthAnchor.constraint(equalToConstant: 390).isActive = true
    stack.addArrangedSubview(label)
    stack.addArrangedSubview(input)
    inputs.append(input)
  }
  stack.layoutSubtreeIfNeeded()
  stack.setFrameSize(NSSize(width: 390, height: CGFloat(fields.count * 57)))
  alert.accessoryView = stack
  NSApp.activate(ignoringOtherApps: true)
  guard alert.runModal() == .alertFirstButtonReturn else { return nil }
  return inputs.map(\.stringValue)
}

@MainActor final class ToolController: NSObject, NSWindowDelegate {
  let request: ToolRequest
  let config: URL
  let transport = NCTransport()
  var window: NSWindow?
  var status: NSTextField?
  var work: Task<Void, Never>?
  var sharesWindow: SharesWindow?
  init(_ request: ToolRequest) {
    self.request = request
    config = URL(fileURLWithPath: request.config)
  }
  func windowWillClose(_ notification: Notification) { cancel() }
  @objc func cancel() {
    work?.cancel()
    finish(ToolResult(success: false, message: "Nextcloud action cancelled.", cancelled: true))
  }
  func progress(_ text: String) {
    let w = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 470, height: 150), styleMask: [.titled, .closable],
      backing: .buffered, defer: false)
    w.title = "Nextcloud — Schnauz"
    w.delegate = self
    w.center()
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.spacing = 16
    let label = NSTextField(wrappingLabelWithString: text)
    label.alignment = .center
    status = label
    let spinner = NSProgressIndicator()
    spinner.style = .spinning
    spinner.startAnimation(nil)
    let button = NSButton(title: "Cancel", target: self, action: #selector(cancel))
    [label, spinner, button].forEach(stack.addArrangedSubview)
    w.contentView = NSView()
    w.contentView!.addSubview(stack)
    stack.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: w.contentView!.leadingAnchor, constant: 22),
      stack.trailingAnchor.constraint(equalTo: w.contentView!.trailingAnchor, constant: -22),
      stack.centerYAnchor.constraint(equalTo: w.contentView!.centerYAnchor),
    ])
    window = w
    w.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
  }
  func start() throws {
    if request.operation == "connect" {
      try connect()
      return
    }
    let config = try NCStore.load(config)
    guard let root = request.root, let account = config.accounts.first(where: { $0.root == root })
    else { throw NCFailure.message("Connect this local folder to Nextcloud first.") }
    if request.operation == "disconnect" {
      let alert = NSAlert()
      alert.messageText = "Disconnect this folder from Schnauz?"
      alert.informativeText =
        "Your files and the Nextcloud desktop client's connection stay in place. Schnauz will remove and revoke only its own access."
      alert.addButton(withTitle: "Disconnect")
      alert.addButton(withTitle: "Cancel")
      NSApp.activate(ignoringOtherApps: true)
      if alert.runModal() != .alertFirstButtonReturn { cancel() }
      work = Task {
        var revoked = false
        if let password = try? NCKeychain.read(account),
          let client = try? NCClient(account: account, password: password, transport: transport)
        {
          revoked = (try? await client.ocs(["core", "apppassword"], method: "DELETE")) != nil
        }
        do {
          try NCStore.update(self.config) { $0.accounts.removeAll { $0.id == account.id } }
          NCKeychain.remove(account.id)
          finish(
            ToolResult(
              success: true,
              message: revoked
                ? "Nextcloud disconnected from Schnauz."
                : "Disconnected locally. Server access could not be revoked; remove Schnauz in Nextcloud's Security settings."
            ))
        } catch { fail(error) }
      }
      return
    }
    let client = try NCClient(
      account: account, password: NCKeychain.read(account), transport: transport)
    if request.operation == "refresh" {
      work = Task {
        do {
          let caps = try await client.ocs(["cloud", "capabilities"]) as? [String: Any] ?? [:]
          let locking = NCClient.supportsLocking(caps)
          try NCStore.update(self.config) { config in
            if let index = config.accounts.firstIndex(where: { $0.id == account.id }) {
              config.accounts[index].locking = locking
            }
          }
          finish(
            ToolResult(
              success: true,
              message:
                "Nextcloud features refreshed. File locking is \(locking ? "available" : "not enabled on this server")."
            ))
        } catch { fail(error) }
      }
      return
    }
    guard let path = request.path else {
      throw NCFailure.message("No Nextcloud item was selected.")
    }
    _ = try NCPaths.remotePath(path, account: account)
    if request.operation == "shares" {
      sharesWindow = SharesWindow(client: client, path: path)
      sharesWindow!.show()
      return
    }
    work = Task {
      do {
        switch request.operation {
        case "open":
          let url = try await client.internalLink(local: path)
          guard NSWorkspace.shared.open(url) else {
            throw NCFailure.message("Could not open the browser.")
          }
          finish(ToolResult(success: true, message: "Opened in Nextcloud."))
        case "copy-link":
          let url = try await client.internalLink(local: path)
          NSPasteboard.general.clearContents()
          NSPasteboard.general.setString(url.absoluteString, forType: .string)
          finish(ToolResult(success: true, message: "Nextcloud internal link copied."))
        case "lock", "unlock":
          try await client.setLock(local: path, locked: request.operation == "lock")
          finish(
            ToolResult(
              success: true,
              message: request.operation == "lock"
                ? "File locked in Nextcloud." : "File unlocked in Nextcloud."))
        default: throw NCFailure.message("Unknown Nextcloud operation.")
        }
      } catch { fail(error) }
    }
  }
  func fail(_ error: Error) -> Never {
    finish(
      ToolResult(
        success: false,
        message: (error as? NCFailure)?.localizedDescription
          ?? "Nextcloud could not finish this action. Check your connection and try again."))
  }
  func connect() throws {
    var selected = request.root
    if selected == nil {
      let panel = NSOpenPanel()
      panel.title = "Choose your local Nextcloud folder"
      panel.canChooseDirectories = true
      panel.canChooseFiles = false
      panel.allowsMultipleSelection = false
      panel.directoryURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(
        "Library/CloudStorage")
      NSApp.activate(ignoringOtherApps: true)
      guard panel.runModal() == .OK, let url = panel.url else {
        cancel()
        return
      }
      selected = url.standardizedFileURL.path
    }
    let root = URL(fileURLWithPath: selected!).standardizedFileURL.path
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: root, isDirectory: &isDirectory),
      isDirectory.boolValue
    else { throw NCFailure.message("Choose an existing local Nextcloud folder.") }
    let existing = try NCStore.load(config).accounts.first { $0.root == root }
    guard
      let fields = prompt(
        "Connect Nextcloud",
        detail:
          "Local folder: \(root)\nSign in through your browser. Schnauz stores its own app credential in macOS Keychain.",
        fields: [
          ("Server address", existing?.server ?? "", false),
          ("Remote folder (usually /)", existing?.remoteRoot ?? "/", false),
        ], confirm: "Open browser login")
    else {
      cancel()
      return
    }
    let base = try NCPaths.server(fields[0])
    let remote = try NCPaths.remoteFolder(fields[1])
    if base.scheme == "http" {
      let alert = NSAlert()
      alert.messageText = "Use an unencrypted HTTP connection?"
      alert.informativeText =
        "This server address sends Nextcloud access and requests without encryption. Use HTTPS if your server supports it."
      alert.addButton(withTitle: "Use HTTP")
      alert.addButton(withTitle: "Cancel")
      if alert.runModal() != .alertFirstButtonReturn { cancel() }
    }
    progress("Preparing Nextcloud browser login…")
    work = Task {
      var storedID: String?
      var pendingAccess: NCClient?
      do {
        let flow = try await NCLogin.begin(server: base, transport: transport)
        status?.stringValue = "Finish signing in and grant Schnauz access in your browser."
        guard NSWorkspace.shared.open(flow.login) else {
          throw NCFailure.message("Could not open the login page.")
        }
        let (server, login, password) = try await flow.wait(transport: transport)
        status?.stringValue = "Checking this folder on Nextcloud…"
        let provisional = NCAccount(
          id: UUID().uuidString, root: root, server: server.absoluteString, login: login,
          userID: login, remoteRoot: remote, locking: false)
        let first = try NCClient(account: provisional, password: password, transport: transport)
        pendingAccess = first
        guard let user = try await first.ocs(["cloud", "user"]) as? [String: Any],
          let id = user["id"] as? String, !id.isEmpty
        else { throw NCFailure.message("Could not identify the Nextcloud user.") }
        var account = NCAccount(
          id: provisional.id, root: root, server: provisional.server, login: login, userID: id,
          remoteRoot: remote, locking: false)
        let client = try NCClient(account: account, password: password, transport: transport)
        _ = try await client.metadata(local: root)
        if let caps = try? await client.ocs(["cloud", "capabilities"]) as? [String: Any] {
          account.locking = NCClient.supportsLocking(caps)
        }
        try Task.checkCancellation()
        try NCKeychain.store(password, account: account)
        storedID = account.id
        try NCStore.update(self.config) { config in
          config.accounts.removeAll { $0.root == root }
          config.accounts.append(account)
        }
        storedID = nil
        pendingAccess = nil
        if let old = existing {
          if let oldPassword = try? NCKeychain.read(old),
            let oldClient = try? NCClient(account: old, password: oldPassword)
          {
            _ = try? await oldClient.ocs(["core", "apppassword"], method: "DELETE")
          }
          NCKeychain.remove(old.id)
        }
        finish(
          ToolResult(
            success: true,
            message: "Nextcloud connected for \(URL(fileURLWithPath: root).lastPathComponent)."))
      } catch {
        if let id = storedID { NCKeychain.remove(id) }
        if let abandoned = pendingAccess {
          _ = try? await abandoned.ocs(["core", "apppassword"], method: "DELETE")
        }
        if error is CancellationError { cancel() }
        fail(error)
      }
    }
  }
}

@MainActor
final class SharesWindow: NSObject, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate {
  let client: NCClient, path: String
  let window: NSWindow, table = NSTableView(),
    label = NSTextField(labelWithString: "Loading shares…")
  var rows: [NCShare] = [], pending = false, isDirectory = false
  var work: Task<Void, Never>?
  var buttons: [NSButton] = []
  init(client: NCClient, path: String) {
    self.client = client
    self.path = path
    window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 620, height: 390),
      styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
    super.init()
    window.title = "Share options — Nextcloud"
    window.delegate = self
    window.minSize = NSSize(width: 580, height: 340)
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 12
    let title = NSTextField(labelWithString: URL(fileURLWithPath: path).lastPathComponent)
    title.font = .boldSystemFont(ofSize: 16)
    let context = NSTextField(
      labelWithString: "\(client.account.login) · \(client.base.host ?? "Nextcloud")")
    context.textColor = .secondaryLabelColor
    let scroll = NSScrollView()
    scroll.hasVerticalScroller = true
    scroll.borderType = .bezelBorder
    let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("share"))
    column.title = "Shared with · access"
    column.width = 550
    table.addTableColumn(column)
    table.dataSource = self
    table.delegate = self
    table.rowHeight = 27
    table.allowsEmptySelection = true
    scroll.documentView = table
    let actions = NSStackView()
    actions.orientation = .horizontal
    actions.spacing = 8
    for (title, action) in [
      ("New link…", #selector(newLink)), ("Share with…", #selector(shareWith)),
      ("Permissions…", #selector(permissions)), ("Copy link", #selector(copyLink)),
      ("Remove…", #selector(removeShare)),
    ] {
      let button = NSButton(title: title, target: self, action: action)
      buttons.append(button)
      actions.addArrangedSubview(button)
    }
    label.font = .systemFont(ofSize: 11)
    label.textColor = .secondaryLabelColor
    label.lineBreakMode = .byTruncatingTail
    [title, context, scroll, actions, label].forEach(stack.addArrangedSubview)
    window.contentView!.addSubview(stack)
    stack.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor, constant: 18),
      stack.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor, constant: -18),
      stack.topAnchor.constraint(equalTo: window.contentView!.topAnchor, constant: 18),
      stack.bottomAnchor.constraint(equalTo: window.contentView!.bottomAnchor, constant: -16),
      scroll.widthAnchor.constraint(equalTo: stack.widthAnchor),
      scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 150),
    ])
  }
  func show() {
    window.center()
    window.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
    refresh()
  }
  func windowWillClose(_ notification: Notification) {
    work?.cancel()
    finish(ToolResult(success: true, message: "Nextcloud sharing closed."))
  }
  func numberOfRows(in tableView: NSTableView) -> Int { rows.count }
  func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView?
  {
    let share = rows[row]
    let field = NSTextField(
      labelWithString:
        "\(share.recipient) · \(share.permissions == 1 ? "Read only" : "Can edit / custom access")")
    field.lineBreakMode = .byTruncatingTail
    return field
  }
  func perform(_ operation: @escaping () async throws -> Void) {
    guard !pending else { return }
    pending = true
    buttons.forEach { $0.isEnabled = false }
    label.stringValue = "Working…"
    work = Task {
      do {
        try await operation()
        rows = try await client.shares(local: path)
        table.reloadData()
        label.stringValue =
          rows.isEmpty
          ? "No shares. Create a link or share with a Nextcloud user or group."
          : "\(rows.count) share(s) · changes are saved to Nextcloud"
      } catch is CancellationError { return } catch {
        label.stringValue = "The action failed. Your previous shares remain listed."
        errorAlert(error)
      }
      pending = false
      buttons.forEach { $0.isEnabled = true }
    }
  }
  func refresh() {
    perform { [self] in
      let info = try await client.metadata(local: path)
      isDirectory = info["collection"] == "true"
    }
  }
  func selected() -> NCShare? {
    let row = table.selectedRow
    if rows.indices.contains(row) { return rows[row] }
    errorAlert(NCFailure.message("Select a share in the list first."))
    return nil
  }
  @objc func newLink() {
    guard
      let fields = prompt(
        "Create a public link",
        detail:
          "Anyone with this new link can read this item. No link is created until you click Create link.",
        fields: [
          ("Password (optional, unless your server requires it)", "", true),
          ("Expiry YYYY-MM-DD (optional)", "", false),
        ], confirm: "Create link")
    else { return }
    if !fields[1].isEmpty {
      let formatter = DateFormatter()
      formatter.locale = Locale(identifier: "en_US_POSIX")
      formatter.dateFormat = "yyyy-MM-dd"
      formatter.isLenient = false
      guard let date = formatter.date(from: fields[1]), formatter.string(from: date) == fields[1]
      else {
        errorAlert(NCFailure.message("Use YYYY-MM-DD for the expiry date."))
        return
      }
    }
    perform { [self] in
      try await client.createShare(local: path, type: 3, linkPassword: fields[0], expiry: fields[1])
    }
  }
  @objc func shareWith() {
    let alert = NSAlert()
    alert.messageText = "Share with a Nextcloud user or group"
    alert.informativeText =
      "Enter their Nextcloud ID. Access starts as read only; you can change it afterwards."
    alert.addButton(withTitle: "Share")
    alert.addButton(withTitle: "Cancel")
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.spacing = 10
    let type = NSPopUpButton()
    type.addItems(withTitles: ["Nextcloud user", "Nextcloud group"])
    let name = NSTextField()
    name.placeholderString = "User or group ID"
    name.widthAnchor.constraint(equalToConstant: 360).isActive = true
    [type, name].forEach(stack.addArrangedSubview)
    stack.setFrameSize(NSSize(width: 360, height: 65))
    alert.accessoryView = stack
    if alert.runModal() == .alertFirstButtonReturn {
      let chosen = type.indexOfSelectedItem
      let value = name.stringValue
      perform { [self] in try await client.createShare(local: path, type: chosen, recipient: value)
      }
    }
  }
  @objc func permissions() {
    guard let share = selected() else { return }
    if share.type == 3 {
      errorAlert(
        NCFailure.message(
          "Public links created here are read only. Use Nextcloud in your browser for upload-only links or other advanced settings."
        ))
      return
    }
    let alert = NSAlert()
    alert.messageText = "Change access for \(share.recipient)"
    alert.informativeText =
      "Edit access allows changes to this item, including creating and deleting contents for a folder."
    alert.addButton(withTitle: "Save")
    alert.addButton(withTitle: "Cancel")
    let picker = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 300, height: 28))
    picker.addItems(withTitles: ["Read only", "Can edit"])
    picker.selectItem(at: share.permissions == 1 ? 0 : 1)
    alert.accessoryView = picker
    if alert.runModal() == .alertFirstButtonReturn {
      let permission = picker.indexOfSelectedItem == 0 ? 1 : (isDirectory ? 15 : 3)
      perform { [self] in try await client.changeShare(share, permissions: permission) }
    }
  }
  @objc func copyLink() {
    guard let share = selected() else { return }
    do {
      guard let value = share.url else {
        throw NCFailure.message(
          "This share has no public link. Use Copy internal link from the file's Nextcloud menu instead."
        )
      }
      let url = try NCPaths.webLink(value)
      NSPasteboard.general.clearContents()
      NSPasteboard.general.setString(url.absoluteString, forType: .string)
      label.stringValue = "Share link copied."
    } catch { errorAlert(error) }
  }
  @objc func removeShare() {
    guard let share = selected() else { return }
    let alert = NSAlert()
    alert.messageText = "Remove this share?"
    alert.informativeText =
      "Access through \(share.recipient) will end. The file or folder stays in place."
    alert.addButton(withTitle: "Remove share")
    alert.addButton(withTitle: "Cancel")
    if alert.runModal() == .alertFirstButtonReturn {
      perform { [self] in try await client.changeShare(share, permissions: nil) }
    }
  }
}

MainActor.assumeIsolated {
  let app = NSApplication.shared
  app.setActivationPolicy(.accessory)
  let parent = getppid()
  // Keep this independent of AppKit's modal event loop.
  let parentMonitor = DispatchSource.makeTimerSource(queue: .global(qos: .utility))
  parentMonitor.schedule(deadline: .now() + 2, repeating: 2)
  parentMonitor.setEventHandler {
    if getppid() != parent { exit(0) }
  }
  parentMonitor.resume()
  if let image = NSImage(
    contentsOfFile: URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
      .appendingPathComponent("../Resources/AppIcon.icns").standardizedFileURL.path)
  {
    app.applicationIconImage = image
  }
  let controller: ToolController
  do {
    let data = FileHandle.standardInput.readDataToEndOfFile()
    guard data.count < 64 * 1024 else { throw NCFailure.message("Invalid Nextcloud request.") }
    controller = ToolController(try JSONDecoder().decode(ToolRequest.self, from: data))
    try controller.start()
  } catch {
    finish(
      ToolResult(
        success: false,
        message: (error as? NCFailure)?.localizedDescription
          ?? "Could not read Nextcloud settings. Existing settings have been preserved."))
  }
  withExtendedLifetime((controller, parentMonitor)) { app.run() }
}
