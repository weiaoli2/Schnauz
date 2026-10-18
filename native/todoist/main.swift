import AppKit
import Foundation

struct TDRequest: Decodable {
  let operation: String
  let config: String
  let account: String?
  let project_id: String?
  let project_name: String?
  let task_id: String?
  let request_id: String?
  let expected_updated: String?
  let allow_create: Bool?
  let palette: [String: UInt32]?
}
struct TDResult: Encodable {
  var success = true
  var message = ""
  var cancelled = false
  var account: TDAccount? = nil
  var project: TDProject? = nil
  var snapshot: TDSnapshot? = nil
  var task: TDTask? = nil
  var retry_after: Int? = nil
}
@MainActor func finish(_ result: TDResult) -> Never {
  if let data = try? JSONEncoder().encode(result) { FileHandle.standardOutput.write(data) }
  exit(result.success || result.cancelled ? 0 : 1)
}
@MainActor func problemAlert(_ error: Error) {
  let alert = NSAlert()
  alert.messageText = "Todoist"
  alert.informativeText =
    (error as? TDProblem)?.text ?? "Todoist could not finish this request. Please try again."
  alert.addButton(withTitle: "OK")
  alert.runModal()
}
@MainActor func prompt(
  _ title: String, detail: String, values: [(String, String, Bool)], confirm: String
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
  for (label, value, secure) in values {
    let caption = NSTextField(labelWithString: label)
    caption.font = .systemFont(ofSize: 11)
    let field = secure ? NSSecureTextField(string: value) : NSTextField(string: value)
    field.widthAnchor.constraint(equalToConstant: 390).isActive = true
    stack.addArrangedSubview(caption)
    stack.addArrangedSubview(field)
    inputs.append(field)
  }
  stack.setFrameSize(NSSize(width: 390, height: CGFloat(values.count * 57)))
  alert.accessoryView = stack
  NSApp.activate(ignoringOtherApps: true)
  guard alert.runModal() == .alertFirstButtonReturn else { return nil }
  return inputs.map(\.stringValue)
}

@MainActor final class TDController: NSObject, NSWindowDelegate {
  let request: TDRequest
  let file: URL
  var work: Task<Void, Never>?
  var window: NSWindow?
  init(_ request: TDRequest) {
    self.request = request
    file = URL(fileURLWithPath: request.config)
  }
  @objc func cancel() {
    work?.cancel()
    finish(TDResult(success: false, message: "Todoist action cancelled.", cancelled: true))
  }
  func windowWillClose(_ notification: Notification) { cancel() }
  func progress(_ message: String, cancellable: Bool = true) {
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 430, height: 125), styleMask: [.titled, .closable],
      backing: .buffered, defer: false)
    window.title = "Todoist — Schnauz"
    window.delegate = self
    window.standardWindowButton(.closeButton)?.isEnabled = cancellable
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.spacing = 14
    stack.addArrangedSubview(NSTextField(labelWithString: message))
    let spinner = NSProgressIndicator()
    spinner.style = .spinning
    spinner.startAnimation(nil)
    stack.addArrangedSubview(spinner)
    if cancellable { stack.addArrangedSubview(NSButton(title: "Cancel", target: self, action: #selector(cancel))) }
    window.contentView!.addSubview(stack)
    stack.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      stack.centerXAnchor.constraint(equalTo: window.contentView!.centerXAnchor),
      stack.centerYAnchor.constraint(equalTo: window.contentView!.centerYAnchor),
    ])
    self.window = window
    window.center()
    window.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
  }
  func start() throws {
    let stored = try TDStore.load(file)
    if request.operation == "connect" {
      guard
        let fields = prompt(
          "Connect Todoist",
          detail:
            "Copy your API token from Todoist Settings → Integrations → Developer. Schnauz stores it only in macOS Keychain.",
          values: [("Todoist API token", "", true)], confirm: "Connect")
      else {
        cancel()
        return
      }
      let token = fields[0].trimmingCharacters(in: .whitespacesAndNewlines)
      let client = try TDClient(token: token)
      progress("Checking your Todoist account…")
      work = Task {
        do {
          let (userID, name) = try await client.identity()
          let same = stored.account?.userID == userID
          let account = TDAccount(
            id: same ? stored.account!.id : UUID().uuidString, userID: userID, name: name)
          try TDKeychain.store(token, account: account)
          do { try TDStore.save(TDConfig(account: account), to: file) } catch {
            if !same { TDKeychain.remove(account.id) }
            throw error
          }
          if let old = stored.account, old.id != account.id { TDKeychain.remove(old.id) }
          finish(TDResult(message: "Todoist connected.", account: account))
        } catch { fail(error) }
      }
      return
    }
    guard let account = stored.account, account.id == request.account else {
      throw TDProblem(text: "Connect this Todoist account in Settings first.")
    }
    if request.operation == "disconnect" {
      let alert = NSAlert()
      alert.messageText = "Disconnect Todoist from Schnauz?"
      alert.informativeText =
        "Your projects and tasks stay in Todoist. Schnauz removes its local access; it does not revoke a personal token that other apps may use."
      alert.addButton(withTitle: "Disconnect")
      alert.addButton(withTitle: "Cancel")
      NSApp.activate(ignoringOtherApps: true)
      guard alert.runModal() == .alertFirstButtonReturn else {
        cancel()
        return
      }
      try TDStore.save(TDConfig(), to: file)
      TDKeychain.remove(account.id)
      finish(TDResult(message: "Todoist disconnected from Schnauz."))
    }
    let client = try TDClient(token: TDKeychain.read(account))
    if request.operation == "ensure-project" {
      guard let name = request.project_name, !name.isEmpty else {
        throw TDProblem(text: "No Schnauz project name was provided.")
      }
      work = Task {
        do {
          let matches = try await client.resolveProject(
            name: name, allowCreate: request.allow_create == true, requestID: request.request_id)
          var selected: TDProject?
          if matches.count == 1 {
            selected = matches[0]
          } else if matches.count > 1 {
            selected = choose(matches, name: name)
          }
          finish(
            TDResult(
              message: selected == nil
                ? "No matching Todoist project found." : "Todoist project linked.",
              project: selected))
        } catch { fail(error) }
      }
      return
    }
    guard let project = request.project_id else {
      throw TDProblem(text: "Link this Schnauz project to Todoist first.")
    }
    work = Task {
      do {
        switch request.operation {
        case "snapshot": finish(TDResult(snapshot: try await client.snapshot(projectID: project)))
        case "complete":
          guard let task = request.task_id, let id = request.request_id else {
            throw TDProblem(text: "No Todoist task was selected.")
          }
          try await client.complete(
            projectID: project, taskID: task, requestID: id,
            expectedUpdated: request.expected_updated)
          finish(TDResult(message: "Task completed in Todoist."))
        case "add": await addTask(client: client, project: project)
        default: throw TDProblem(text: "Unknown Todoist action.")
        }
      } catch { fail(error) }
    }
  }
  func choose(_ projects: [TDProject], name: String) -> TDProject? {
    let alert = NSAlert()
    alert.messageText = "Choose the Todoist project named ‘\(name)’"
    alert.informativeText =
      "Todoist has several projects with this name. Their parent projects help distinguish them. You can open the selected project to check its tasks before linking."
    alert.addButton(withTitle: "Link project")
    alert.addButton(withTitle: "Cancel")
    alert.addButton(withTitle: "View selected project")
    let picker = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 390, height: 28))
    picker.addItems(
      withTitles: projects.enumerated().map {
        "\($0.element.display_path ?? $0.element.name) (\($0.offset + 1))"
      })
    alert.accessoryView = picker
    NSApp.activate(ignoringOtherApps: true)
    while true {
      let choice = alert.runModal()
      if choice == .alertFirstButtonReturn { return projects[picker.indexOfSelectedItem] }
      if choice == .alertThirdButtonReturn {
        let id = projects[picker.indexOfSelectedItem].id
        var native = URLComponents(string: "todoist://project")!
        native.queryItems = [URLQueryItem(name: "id", value: id)]
        if NSWorkspace.shared.urlForApplication(toOpen: native.url!) == nil || !NSWorkspace.shared.open(native.url!) {
          NSWorkspace.shared.open(URL(string: "https://app.todoist.com/app/project")!.appendingPathComponent(id))
        }
      } else {
        cancel()
        return nil
      }
    }
  }
  func addTask(client: TDClient, project: String) async -> Never {
    // Read the captured destination first; the composer never offers a project picker.
    let destination: TDProject
    do { destination = try await client.project(project) } catch { fail(error) }
    var draft = TDDraft()
    let creationID = request.request_id ?? UUID().uuidString
    let reminderID = UUID().uuidString, locationID = UUID().uuidString
    var previous = ""
    var currentID = creationID
    var savedTask: TDTask? = nil
    var reminderSaved = false, locationSaved = false
    var message = ""
    while true {
      let composer = TDComposer(project: destination.name, draft: draft, palette: request.palette,
        message: message, saved: savedTask != nil)
      guard let entered = composer.present() else {
        if let task = savedTask {
          finish(TDResult(message: "Task saved in \(destination.name). Optional reminders were not completed.", task: task))
        }
        cancel()
        continue
      }
      draft = entered
      let signature = "\(draft.content)\u{0}\(draft.description)\u{0}\(draft.due)\u{0}\(draft.labels)\u{0}\(draft.priority)\u{0}\(String(describing: draft.deadline))"
      if !previous.isEmpty && signature != previous { currentID = UUID().uuidString }
      previous = signature
      do {
        progress(savedTask == nil ? "Saving task to \(destination.name)…" : "Saving reminder options…", cancellable: false)
        if savedTask == nil {
          if let date = draft.reminder, date <= Date() { throw TDProblem(text: "Choose a reminder time in the future.") }
          savedTask = try await client.add(projectID: project, content: draft.content,
            due: draft.due, section: nil, requestID: currentID, options: draft.options)
        }
        let task = savedTask!
        if let date = draft.reminder, !reminderSaved {
          try await client.reminder(taskID: task.id, date: date, requestID: reminderID)
          reminderSaved = true
        }
        if let location = draft.location, !locationSaved {
          try await client.locationReminder(taskID: task.id, projectID: project, location: location, requestID: locationID)
          locationSaved = true
        }
        finish(TDResult(message: "Task added to \(destination.name).", task: task))
      } catch {
        window?.orderOut(nil); window = nil
        let problem = error as? TDProblem
        let detail = problem?.text ?? "Todoist did not confirm the change. Retry the same request to avoid duplicates."
        message = savedTask == nil ? detail : "Task saved. Reminder options failed: \(detail) Retry options will not create another task."
        if let delay = problem?.retryAfter {
          progress("Waiting \(delay)s before retrying Todoist…")
          do { try await Task.sleep(nanoseconds: UInt64(delay) * 1_000_000_000) } catch { cancel() }
          window?.orderOut(nil); window = nil
        }
      }
    }
  }
  func fail(_ error: Error) -> Never {
    if error is CancellationError { cancel() }
    let problem = error as? TDProblem
    finish(
      TDResult(
        success: false,
        message: problem?.text
          ?? "Todoist could not finish this action. Your settings and cached tasks have been kept.",
        retry_after: problem?.retryAfter))
  }
}

MainActor.assumeIsolated {
  let app = NSApplication.shared
  app.setActivationPolicy(.accessory)
  // Standard responder-chain shortcuts also work in the secure token field.
  let mainMenu = NSMenu()
  let editItem = NSMenuItem()
  let edit = NSMenu(title: "Edit")
  for (title, action, key) in [
    ("Undo", "undo:", "z"), ("Cut", "cut:", "x"), ("Copy", "copy:", "c"), ("Paste", "paste:", "v"),
    ("Select All", "selectAll:", "a"),
  ] {
    edit.addItem(NSMenuItem(title: title, action: Selector(action), keyEquivalent: key))
  }
  editItem.submenu = edit
  mainMenu.addItem(editItem)
  app.mainMenu = mainMenu
  if let icon = NSImage(
    contentsOfFile: URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
      .appendingPathComponent("../Resources/AppIcon.icns").standardizedFileURL.path)
  {
    app.applicationIconImage = icon
  }

  let parent = getppid()
  let monitor = DispatchSource.makeTimerSource(queue: .global(qos: .utility))
  monitor.schedule(deadline: .now() + 2, repeating: 2)
  monitor.setEventHandler { if getppid() != parent { exit(0) } }
  monitor.resume()
  let controller: TDController
  do {
    let data = FileHandle.standardInput.readDataToEndOfFile()
    guard data.count <= 65536 else { throw TDProblem(text: "Invalid Todoist request.") }
    controller = TDController(try JSONDecoder().decode(TDRequest.self, from: data))
    try controller.start()
  } catch {
    finish(
      TDResult(
        success: false,
        message: (error as? TDProblem)?.text
          ?? "Could not read Todoist settings. The existing file has been preserved."))
  }
  withExtendedLifetime((controller, monitor)) { app.run() }
}
