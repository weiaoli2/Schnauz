import AppKit

@MainActor func problemAlert(_ error: Error) { fatalError("No interactive alerts in the offline layout check") }
@main struct ComposerLayoutTests {
  @MainActor static func main() {
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)

@MainActor func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
for dark in [false, true] {
  var draft = TDDraft()
  draft.content = "Review the launch checklist"
  draft.description = "Sample task; no account connection or API writes."
  draft.labels = "website, launch"
  let palette: [String: UInt32] = ["bg": dark ? 0x191b26 : 0xf8f9fc,
    "sidebar": dark ? 0x202333 : 0xf0f0f6, "text": dark ? 0xeeeeff : 0,
    "accent": 0x5964ee, "border": 0xdedee7]
  let form = TDComposer(project: "Project Alpha", draft: draft, palette: palette, message: "", saved: false)
  let content = form.window.contentView!
  content.layoutSubtreeIfNeeded()
  precondition(form.window.frame.height <= 410, "Composer must stay compact")
  precondition(form.name.frame.width > 450, "Task title needs readable width")
  precondition(form.date.frame.width >= 150 && form.date.frame.height >= 24, "Date control must not collapse")
  precondition(form.details.frame.width > 400, "Description text must have a usable width")
  precondition(form.details.insertionPointColor.isEqual(form.ink), "Description caret follows the themed text color")
  for field in [form.name, form.date, form.labels] {
    let editor = form.window.fieldEditor(true, for: field) as! NSTextView
    precondition(editor === form.inputEditor && editor.isFieldEditor, "Editable fields use the owned field editor")
    precondition(editor.insertionPointColor.isEqual(form.ink), "Field-editor caret contrasts with the themed background")
    // Simulate AppKit resetting editor attributes between fields; assigning
    // the field editor again must restore the visible caret.
    editor.insertionPointColor = .clear
    _ = form.windowWillReturnFieldEditor(form.window, to: field)
    precondition(editor.insertionPointColor.isEqual(form.ink))
  }
  form.details.insertionPointColor = .clear
  form.textDidBeginEditing(Notification(name: NSText.didBeginEditingNotification, object: form.details))
  precondition(form.details.insertionPointColor.isEqual(form.ink), "Description focus restores its caret color")
  precondition(form.name.stringValue == draft.content && form.details.string == draft.description,
    "Caret styling preserves the task draft")
  let project = descendants(content).compactMap { $0 as? NSTextField }.first { $0.stringValue == "# Project Alpha" }!
  precondition(!project.isEditable && project.frame.width >= 150, "Captured project stays readable and locked")
  precondition(form.priority.indexOfSelectedItem == 3, "Default P4 matches API's normal priority")
  precondition(draft.options.labels == ["website", "launch"], "Form labels feed real task options")
  let retry = TDComposer(project: "Project Alpha", draft: draft, palette: palette,
    message: "Task saved. Retry options will not create another task.", saved: true)
  retry.window.contentView!.layoutSubtreeIfNeeded()
  precondition(!retry.name.isEnabled && !retry.date.isEnabled && !retry.priority.isEnabled,
    "A partial-save retry cannot change the task's destination/details")
  precondition(retry.submit.title == "Retry options", "Retry is clearly distinguished from new creation")
  precondition(!retry.inputEditor.isEditable && !retry.details.isEditable,
    "Caret styling must not unlock saved task fields")
}
print("PASS compact light/dark composer layout, themed title/description carets, locked project, readable fields and partial-save retry state; no windows shown or accounts contacted")
  }
}
