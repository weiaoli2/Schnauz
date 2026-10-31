import AppKit
import MapKit

// The same sitting mascot as src/mascot.rs, drawn as vector shapes rather than
// substituting the app icon. Coordinates and colours match the window mascot.
@MainActor final class TDDogView: NSView {
  override var isFlipped: Bool { true }
  override func draw(_ dirtyRect: NSRect) {
    let scale = bounds.width / 32
    func color(_ value: UInt32) -> NSColor { TDComposer.color(value) }
    func oval(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ c: UInt32) {
      color(c).setFill()
      NSBezierPath(roundedRect: NSRect(x: x*scale, y: y*scale, width: w*scale, height: h*scale),
                   xRadius: min(w,h)*scale/2, yRadius: min(w,h)*scale/2).fill()
    }
    func line(_ points: [(CGFloat,CGFloat)], _ width: CGFloat, _ c: UInt32) {
      let path = NSBezierPath(); path.lineWidth = width*scale; path.lineCapStyle = .round
      path.move(to: NSPoint(x: points[0].0*scale, y: points[0].1*scale))
      for p in points.dropFirst() { path.line(to: NSPoint(x: p.0*scale, y: p.1*scale)) }
      color(c).setStroke(); path.stroke()
    }
    func triangle(_ points: [(CGFloat,CGFloat)], _ c: UInt32) {
      let path = NSBezierPath()
      path.move(to: NSPoint(x: points[0].0*scale, y: points[0].1*scale))
      for p in points.dropFirst() { path.line(to: NSPoint(x: p.0*scale, y: p.1*scale)) }
      path.close(); color(c).setFill(); path.fill()
    }
    let blue: UInt32 = 0x6286bd, dark: UInt32 = 0x294164, cream: UInt32 = 0xf5eedc
    oval(3,29,26,1.5,0xabb7c7); line([(10,24),(5,25),(4,22)],2.4,dark)
    oval(9,17,14,12,blue); oval(8,25,7,4,dark); oval(19,25,6,4,dark)
    line([(16,19),(15,27),(18,27)],2.8,cream)
    triangle([(11,10),(12,3),(17,6)],dark); triangle([(19,6),(24,4),(24,12)],dark)
    oval(12,5,12,13,blue); oval(11,13,14,8,cream)
    oval(13,9,3,2,cream); oval(19,9,3,2,cream)
    oval(14,11,1.8,2.2,dark); oval(20,11,1.8,2.2,dark)
    oval(16,14,5,3,dark); line([(18.5,17),(18.5,19)],1.2,dark)
  }
}

struct TDDraft {
  var content = ""
  var description = ""
  var due = ""
  var deadline: Date? = nil
  var priority = 1
  var labels = ""
  var reminder: Date? = nil
  var location: TDLocation? = nil

  var options: TDTaskOptions {
    let formatter = DateFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.calendar = Calendar(identifier: .gregorian)
    formatter.dateFormat = "yyyy-MM-dd"
    return TDTaskOptions(description: description,
      deadline: deadline.map(formatter.string(from:)), priority: priority,
      labels: labels.split(separator: ",").map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty })
  }
}

@MainActor final class TDComposer: NSObject, NSWindowDelegate, NSTextFieldDelegate, NSTextViewDelegate {
  static func color(_ value: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat((value >> 16) & 255)/255,
      green: CGFloat((value >> 8) & 255)/255, blue: CGFloat(value & 255)/255, alpha: 1)
  }
  let window: NSWindow
  var draft: TDDraft
  let name = NSTextField()
  let details = NSTextView()
  // NSTextField draws typed text through the window's shared NSTextView.
  // Keep our own editor so its caret follows this form's actual theme ink.
  let inputEditor = NSTextView()
  let date = NSTextField()
  let labels = NSTextField()
  let priority = NSPopUpButton()
  let deadline = NSButton()
  let reminder = NSButton()
  let location = NSButton()
  let submit = NSButton()
  let status = NSTextField(wrappingLabelWithString: "")
  var accepted = false
  let bg: NSColor, surface: NSColor, ink: NSColor, accent: NSColor, border: NSColor
  let saved: Bool

  init(project: String, draft: TDDraft, palette: [String: UInt32]?, message: String, saved: Bool) {
    self.draft = draft; self.saved = saved
    bg = Self.color(palette?["bg"] ?? 0xf8f9fc)
    surface = Self.color(palette?["sidebar"] ?? 0xf0f0f6)
    ink = Self.color(palette?["text"] ?? 0)
    accent = Self.color(palette?["accent"] ?? 0x5964ee)
    border = Self.color(palette?["border"] ?? 0xdedee7)
    window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 380),
      styleMask: [.titled, .closable], backing: .buffered, defer: false)
    super.init()
    window.title = "New task · Schnauz"
    window.titleVisibility = .hidden; window.titlebarAppearsTransparent = true
    window.backgroundColor = bg; window.isReleasedWhenClosed = false; window.delegate = self
    let rgb = (palette?["bg"] ?? 0xf8f9fc)
    let luminance = ((rgb >> 16) & 255)*299 + ((rgb >> 8) & 255)*587 + (rgb & 255)*114
    window.appearance = NSAppearance(named: luminance < 128000 ? .darkAqua : .aqua)
    inputEditor.isFieldEditor = true; inputEditor.isRichText = false
    inputEditor.drawsBackground = false; inputEditor.backgroundColor = bg
    inputEditor.textColor = ink; inputEditor.insertionPointColor = ink
    inputEditor.isEditable = !saved
    build(project: project, message: message)
  }

  func windowWillReturnFieldEditor(_ sender: NSWindow, to client: Any?) -> Any? {
    guard sender === window, let field = client as? NSTextField,
          [name, date, labels].contains(where: { $0 === field }), field.isEnabled else { return nil }
    inputEditor.insertionPointColor = ink
    return inputEditor
  }

  func controlTextDidBeginEditing(_ notification: Notification) {
    guard let field = notification.object as? NSTextField,
          let editor = field.currentEditor() as? NSTextView else { return }
    // AppKit configures the shared editor when focus moves between fields.
    editor.insertionPointColor = ink
    editor.updateInsertionPointStateAndRestartTimer(true)
  }

  func textDidBeginEditing(_ notification: Notification) {
    guard let editor = notification.object as? NSTextView, editor === details else { return }
    editor.insertionPointColor = ink
    editor.updateInsertionPointStateAndRestartTimer(true)
  }

  func label(_ value: String, size: CGFloat = 12, bold: Bool = false) -> NSTextField {
    let field = NSTextField(labelWithString: value)
    field.font = bold ? .boldSystemFont(ofSize: size) : .systemFont(ofSize: size)
    field.textColor = ink; field.lineBreakMode = .byTruncatingTail
    return field
  }
  func constrain(_ view: NSView, in parent: NSView, top: CGFloat, left: CGFloat, right: CGFloat, height: CGFloat) {
    view.translatesAutoresizingMaskIntoConstraints = false; parent.addSubview(view)
    NSLayoutConstraint.activate([
      view.topAnchor.constraint(equalTo: parent.topAnchor, constant: top),
      view.leadingAnchor.constraint(equalTo: parent.leadingAnchor, constant: left),
      view.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -right),
      view.heightAnchor.constraint(equalToConstant: height)])
  }
  func field(_ input: NSTextField, value: String, placeholder: String) {
    input.stringValue = value; input.placeholderString = placeholder
    input.font = .systemFont(ofSize: 12); input.textColor = ink
    input.backgroundColor = bg; input.isBezeled = false; input.drawsBackground = false
    input.focusRingType = .none; input.isEnabled = !saved
    input.delegate = self
  }
  func optionButton(_ button: NSButton, title: String, action: Selector) {
    button.title = title; button.target = self; button.action = action
    button.isBordered = false; button.alignment = .left
    button.font = .systemFont(ofSize: 12); button.contentTintColor = accent
    button.isEnabled = !saved
  }
  func build(project: String, message: String) {
    let root = window.contentView!
    let dog = TDDogView(frame: .zero)
    constrain(dog, in: root, top: 6, left: 18, right: 750, height: 32)
    constrain(label("New task", size: 14, bold: true), in: root, top: 10, left: 60, right: 300, height: 24)
    let rule = NSBox(); rule.boxType = .separator
    constrain(rule, in: root, top: 44, left: 0, right: 0, height: 1)
    let left = NSView(); constrain(left, in: root, top: 45, left: 0, right: 264, height: 277)
    let right = NSView(); right.wantsLayer = true; right.layer?.backgroundColor = surface.cgColor
    constrain(right, in: root, top: 45, left: 536, right: 0, height: 277)
    field(name, value: draft.content, placeholder: "Task name")
    name.font = .boldSystemFont(ofSize: 20)
    constrain(name, in: left, top: 18, left: 20, right: 20, height: 32)
    constrain(label("Description", size: 11), in: left, top: 62, left: 22, right: 20, height: 16)
    let scroll = NSScrollView(); scroll.drawsBackground = true; scroll.backgroundColor = bg
    scroll.hasVerticalScroller = true; scroll.autohidesScrollers = true
    details.string = draft.description; details.font = .systemFont(ofSize: 12)
    details.textColor = ink; details.backgroundColor = bg; details.isRichText = false
    details.insertionPointColor = ink; details.delegate = self
    details.isEditable = !saved; details.isSelectable = true
    details.isVerticallyResizable = true; details.isHorizontallyResizable = false
    details.frame = NSRect(x: 0, y: 0, width: 490, height: 174)
    details.minSize = NSSize(width: 0, height: 174)
    details.autoresizingMask = [.width]; details.textContainerInset = NSSize(width: 4, height: 5)
    details.textContainer?.widthTracksTextView = true
    details.textContainer?.containerSize = NSSize(width: 490, height: CGFloat.greatestFiniteMagnitude)
    scroll.documentView = details
    constrain(scroll, in: left, top: 85, left: 18, right: 18, height: 174)

    let projectName = label("# " + project, bold: true)
    projectName.toolTip = "Fixed to the current Schnauz project; cannot be changed here."
    field(date, value: draft.due, placeholder: "e.g. tomorrow 09:00")
    field(labels, value: draft.labels, placeholder: "Comma-separated labels")
    priority.addItems(withTitles: ["P1 · Urgent", "P2 · High", "P3 · Medium", "P4 · Normal"])
    priority.selectItem(at: 4 - draft.priority); priority.isEnabled = !saved
    priority.font = .systemFont(ofSize: 12)
    optionButton(deadline, title: "No deadline", action: #selector(chooseDeadline))
    optionButton(reminder, title: "No reminder", action: #selector(chooseReminder))
    optionButton(location, title: "No location", action: #selector(chooseLocation))
    let rows: [(String, NSView)] = [("Project", projectName), ("Date", date),
      ("Deadline", deadline), ("Priority", priority), ("Labels", labels),
      ("Reminder", reminder), ("Location", location)]
    for (i, row) in rows.enumerated() {
      let y = CGFloat(12 + i*36)
      constrain(label(row.0, size: 11), in: right, top: y+3, left: 12, right: 178, height: 24)
      constrain(row.1, in: right, top: y, left: 86, right: 12, height: 28)
    }
    let footerRule = NSBox(); footerRule.boxType = .separator
    constrain(footerRule, in: root, top: 322, left: 0, right: 0, height: 1)
    status.stringValue = message.isEmpty ? "Project is fixed · ⌘Return to add" : message
    status.font = .systemFont(ofSize: 11); status.textColor = ink
    constrain(status, in: root, top: 333, left: 20, right: 225, height: 35)
    let cancel = NSButton(title: saved ? "Close" : "Cancel", target: self, action: #selector(cancel))
    cancel.bezelStyle = .rounded; cancel.keyEquivalent = "\u{1b}"
    constrain(cancel, in: root, top: 335, left: 584, right: 122, height: 30)
    submit.title = saved ? "Retry options" : "Add task"; submit.target = self; submit.action = #selector(save)
    submit.bezelStyle = .rounded; submit.bezelColor = accent; submit.contentTintColor = .white
    submit.keyEquivalent = "\r"; submit.keyEquivalentModifierMask = [.command]
    constrain(submit, in: root, top: 335, left: 686, right: 16, height: 30)
    updateOptions()
  }

  func updateOptions() {
    let fmt = DateFormatter(); fmt.dateStyle = .medium; fmt.timeStyle = .none
    deadline.title = draft.deadline.map(fmt.string(from:)) ?? "No deadline"
    fmt.dateStyle = .short; fmt.timeStyle = .short
    reminder.title = draft.reminder.map(fmt.string(from:)) ?? "No reminder"
    location.title = draft.location?.name ?? "No location"
    location.toolTip = draft.location.map { "\($0.latitude), \($0.longitude) · \($0.trigger == "on_enter" ? "On arrival" : "On leaving")" }
  }
  func chooseDate(_ title: String, current: Date?, time: Bool) -> Date?? {
    let alert = NSAlert(); alert.messageText = title
    alert.addButton(withTitle: "Set"); alert.addButton(withTitle: "Cancel"); alert.addButton(withTitle: "Clear")
    let picker = NSDatePicker(frame: NSRect(x: 0, y: 0, width: 300, height: 30))
    picker.datePickerStyle = .textFieldAndStepper
    picker.datePickerElements = time ? [.yearMonthDay, .hourMinute] : [.yearMonthDay]
    picker.dateValue = current ?? Date().addingTimeInterval(time ? 3600 : 0)
    alert.accessoryView = picker
    switch alert.runModal() {
    case .alertFirstButtonReturn: return .some(picker.dateValue)
    case .alertThirdButtonReturn: return .some(nil)
    default: return nil
    }
  }
  @objc func chooseDeadline() {
    if let value = chooseDate("Deadline", current: draft.deadline, time: false) { draft.deadline = value; updateOptions() }
  }
  @objc func chooseReminder() {
    if let value = chooseDate("Remind me at", current: draft.reminder, time: true) { draft.reminder = value; updateOptions() }
  }
  @objc func chooseLocation() {
    let alert = NSAlert(); alert.messageText = "Location reminder"
    alert.informativeText = "Search an address or place using Apple Maps. Location reminders depend on your Todoist plan."
    alert.addButton(withTitle: "Find place"); alert.addButton(withTitle: "Cancel"); alert.addButton(withTitle: "Clear")
    let input = NSTextField(string: draft.location?.name ?? "")
    input.placeholderString = "Address or place"; input.frame = NSRect(x: 0, y: 0, width: 350, height: 26)
    alert.accessoryView = input
    let response = alert.runModal()
    if response == .alertThirdButtonReturn { draft.location = nil; updateOptions(); return }
    guard response == .alertFirstButtonReturn, !input.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
    let query = input.stringValue
    location.isEnabled = false; submit.isEnabled = false; location.title = "Finding place…"
    Task { @MainActor in
      defer { location.isEnabled = !saved; submit.isEnabled = true; updateOptions() }
      let request = MKLocalSearch.Request(); request.naturalLanguageQuery = query
      do {
        let response = try await MKLocalSearch(request: request).start()
        guard window.isVisible else { return }
        let places = Array(response.mapItems.prefix(10))
        guard !places.isEmpty else { throw TDProblem(text: "No matching place found. Try a more complete address.") }
        let pick = NSAlert(); pick.messageText = "Choose the reminder location"
        pick.addButton(withTitle: "Use location"); pick.addButton(withTitle: "Cancel")
        let list = NSPopUpButton(frame: NSRect(x: 0, y: 34, width: 390, height: 26))
        list.addItems(withTitles: places.map { "\($0.name ?? query) — \($0.placemark.title ?? "")" })
        let trigger = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 390, height: 26))
        trigger.addItems(withTitles: ["On arrival · 100 m", "On leaving · 100 m"])
        let box = NSView(frame: NSRect(x: 0, y: 0, width: 390, height: 60)); box.addSubview(list); box.addSubview(trigger)
        pick.accessoryView = box
        if pick.runModal() == .alertFirstButtonReturn {
          let item = places[list.indexOfSelectedItem]; let coordinate = item.placemark.coordinate
          draft.location = TDLocation(name: item.name ?? query, latitude: String(coordinate.latitude),
            longitude: String(coordinate.longitude), trigger: trigger.indexOfSelectedItem == 0 ? "on_enter" : "on_leave", radius: 100)
        }
      } catch { if !window.isVisible { return }; problemAlert((error as? TDProblem) ?? TDProblem(text: "Couldn't search Apple Maps. Try again or leave Location empty.")) }
    }
  }
  @objc func save() {
    guard saved || !name.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      status.stringValue = "Enter a task name."; window.makeFirstResponder(name); return
    }
    draft.content = name.stringValue; draft.description = details.string; draft.due = date.stringValue
    draft.labels = labels.stringValue; draft.priority = 4 - priority.indexOfSelectedItem
    accepted = true; NSApp.stopModal(withCode: .OK); window.orderOut(nil)
  }
  @objc func cancel() { NSApp.stopModal(withCode: .cancel); window.orderOut(nil) }
  func windowShouldClose(_ sender: NSWindow) -> Bool { cancel(); return false }
  func present() -> TDDraft? {
    window.center(); window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
    if !saved { window.makeFirstResponder(name) }
    NSApp.runModal(for: window)
    return accepted ? draft : nil
  }
}
