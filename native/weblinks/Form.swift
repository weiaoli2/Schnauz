import AppKit

struct WebLink: Codable, Equatable {
    var id: String
    var name: String
    var url: String
    var browser: String?
}
struct BrowserChoice: Equatable { var name: String; var path: String }

enum WebLinkValidation: Error { case invalidName, invalidURL }
func normalized(_ link: WebLink) throws -> WebLink {
    var value=link
    value.name=value.name.trimmingCharacters(in:.whitespacesAndNewlines)
    value.url=value.url.trimmingCharacters(in:.whitespacesAndNewlines)
    guard !value.name.isEmpty, value.name.count<=200,
          value.name.rangeOfCharacter(from:.controlCharacters)==nil else {throw WebLinkValidation.invalidName}
    guard value.url.utf8.count<=8192,
          value.url.rangeOfCharacter(from:.whitespacesAndNewlines.union(.controlCharacters))==nil,
          let components=URLComponents(string:value.url),
          ["http","https"].contains(components.scheme?.lowercased() ?? ""),
          let host=components.host, !host.isEmpty, components.url != nil else {throw WebLinkValidation.invalidURL}
    return value
}

@MainActor func installedBrowsers() -> [BrowserChoice] {
    var choices:[String:BrowserChoice]=[:]
    for scheme in ["https","http"] {
        for app in NSWorkspace.shared.urlsForApplications(toOpen:URL(string:"\(scheme)://example.org/")!) {
            let path=app.standardizedFileURL.path
            choices[path]=BrowserChoice(name:app.deletingPathExtension().lastPathComponent,path:path)
        }
    }
    return choices.values.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
}

@MainActor final class WebLinkForm {
    let view=NSView(frame:NSRect(x:0,y:0,width:420,height:204))
    let name=NSTextField(frame:NSRect(x:0,y:154,width:420,height:24))
    let url=NSTextField(frame:NSRect(x:0,y:94,width:420,height:24))
    let browser=NSPopUpButton(frame:NSRect(x:0,y:30,width:420,height:26))
    let error=NSTextField(labelWithString:"")
    let original:WebLink
    var browsers:[BrowserChoice]
    init(_ link:WebLink, browsers:[BrowserChoice]) {
        original=link; self.browsers=browsers
        name.stringValue=link.name;name.placeholderString="Name"
        url.stringValue=link.url;url.placeholderString="https://example.org"
        for (title,y) in [("Name",184.0),("Link",124.0),("Browser for this link",62.0)] {
            let label=NSTextField(labelWithString:title)
            label.frame=NSRect(x:0,y:y,width:420,height:16)
            label.font = .systemFont(ofSize:11)
            view.addSubview(label)
        }
        browser.addItem(withTitle:"macOS Default")
        if let saved=link.browser, !self.browsers.contains(where:{$0.path==saved}) {
            self.browsers.append(BrowserChoice(name:"\(URL(fileURLWithPath:saved).deletingPathExtension().lastPathComponent) (unavailable)",path:saved))
        }
        for choice in self.browsers { browser.addItem(withTitle:choice.name) }
        if let saved=link.browser, let index=self.browsers.firstIndex(where:{$0.path==saved}) { browser.selectItem(at:index+1) }
        name.nextKeyView=url;url.nextKeyView=browser;browser.nextKeyView=name
        error.frame=NSRect(x:0,y:0,width:420,height:22)
        error.font = .systemFont(ofSize:11);error.textColor = .systemRed
        view.addSubview(name);view.addSubview(url);view.addSubview(browser);view.addSubview(error)
    }
    func value() throws -> WebLink {
        let index=browser.indexOfSelectedItem
        return try normalized(WebLink(id:original.id,name:name.stringValue,url:url.stringValue,browser:index>0 ? browsers[index-1].path:nil))
    }
}

@MainActor func editWebLink(_ link:WebLink) -> WebLink? {
    let form=WebLinkForm(link,browsers:installedBrowsers())
    let alert=NSAlert()
    alert.messageText=link.name.isEmpty ? "Add weblink" : "Edit weblink"
    alert.informativeText="Choose a browser for this bookmark. Your macOS default stays unchanged."
    alert.addButton(withTitle:"Save");alert.addButton(withTitle:"Cancel")
    alert.accessoryView=form.view
    alert.window.initialFirstResponder=form.name
    NSApp.activate(ignoringOtherApps:true)
    while alert.runModal() == .alertFirstButtonReturn {
        do {return try form.value()} catch WebLinkValidation.invalidName {
            form.error.stringValue="Enter a name (up to 200 characters)."
            alert.window.makeFirstResponder(form.name)
        } catch {
            form.error.stringValue="Enter a complete http:// or https:// web address."
            alert.window.makeFirstResponder(form.url)
        }
    }
    return nil
}

@MainActor func installEditingMenu() {
    let menu=NSMenu()
    let root=NSMenuItem(title:"Edit",action:nil,keyEquivalent:"")
    let edit=NSMenu(title:"Edit")
    for (title,action,key) in [("Undo","undo:","z"),("Cut","cut:","x"),("Copy","copy:","c"),("Paste","paste:","v"),("Select All","selectAll:","a")] {
        edit.addItem(NSMenuItem(title:title,action:Selector(action),keyEquivalent:key))
    }
    root.submenu=edit;menu.addItem(root);NSApp.mainMenu=menu
}

struct FolderLabel: Codable, Equatable {
    var alias: String
    var add_on: String
}
enum FolderLabelValidation: Error { case invalidText }
@MainActor final class FolderLabelForm {
    let view = NSView(frame: NSRect(x: 0, y: 0, width: 420, height: 142))
    let alias = NSTextField(frame: NSRect(x: 0, y: 90, width: 420, height: 24))
    let addOn = NSTextField(frame: NSRect(x: 0, y: 30, width: 420, height: 24))
    let error = NSTextField(labelWithString: "")
    init(_ label: FolderLabel) {
        alias.stringValue = label.alias
        alias.placeholderString = "Leave empty to use the original name"
        addOn.stringValue = label.add_on
        addOn.placeholderString = "Optional text in brackets after the name"
        for (title, y) in [("Alias", 120.0), ("Add-on", 60.0)] {
            let caption = NSTextField(labelWithString: title)
            caption.frame = NSRect(x: 0, y: y, width: 420, height: 16)
            caption.font = .systemFont(ofSize: 11)
            view.addSubview(caption)
        }
        alias.nextKeyView = addOn; addOn.nextKeyView = alias
        error.frame = NSRect(x: 0, y: 0, width: 420, height: 22)
        error.font = .systemFont(ofSize: 11); error.textColor = .systemRed
        for field in [alias, addOn, error] { view.addSubview(field) }
    }
    func value() throws -> FolderLabel {
        let values = [alias.stringValue, addOn.stringValue].map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        guard values.allSatisfy({ $0.unicodeScalars.count <= 200 && $0.rangeOfCharacter(from: .controlCharacters) == nil }) else {
            throw FolderLabelValidation.invalidText
        }
        return FolderLabel(alias: values[0], add_on: values[1])
    }
}
@MainActor func editFolderLabel(_ label: FolderLabel) -> FolderLabel? {
    let form = FolderLabelForm(label)
    let alert = NSAlert()
    alert.messageText = "Edit alias and add-on"
    alert.informativeText = "The alias replaces the name in the sidebar. An add-on appears in brackets: Name (Add-on)."
    alert.addButton(withTitle: "Save"); alert.addButton(withTitle: "Cancel")
    alert.accessoryView = form.view
    alert.window.initialFirstResponder = form.alias
    NSApp.activate(ignoringOtherApps: true)
    while alert.runModal() == .alertFirstButtonReturn {
        do { return try form.value() } catch {
            form.error.stringValue = "Use up to 200 characters per field, without line breaks."
        }
    }
    return nil
}
