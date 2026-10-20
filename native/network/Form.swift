import AppKit

@MainActor final class NetworkForm {
    let view = NSView(frame: NSRect(x: 0, y: 0, width: 440, height: 296))
    let protocolChoice = NSPopUpButton(frame: NSRect(x: 0, y: 246, width: 440, height: 26))
    let address = NSTextField(frame: NSRect(x: 0, y: 190, width: 440, height: 24))
    let username = NSTextField(frame: NSRect(x: 0, y: 134, width: 440, height: 24))
    let password = NSSecureTextField(frame: NSRect(x: 0, y: 78, width: 440, height: 24))
    let guest = NSButton(checkboxWithTitle: "Connect as Guest", target: nil, action: nil)
    let readOnly = NSButton(checkboxWithTitle: "Read-only", target: nil, action: nil)
    let chooseFolder = NSButton(checkboxWithTitle: "Choose a folder after connecting", target: nil, action: nil)
    let remember = NSButton(checkboxWithTitle: "Remember password in Keychain for Favourites", target: nil, action: nil)
    init(_ saved: NetworkLocation?) {
        protocolChoice.addItems(withTitles: ["SMB · Windows / NAS", "AFP · Mac"])
        if saved?.url.hasPrefix("afp:") == true { protocolChoice.selectItem(at: 1) }
        address.stringValue = saved?.url ?? ""
        address.placeholderString = "192.168.1.20 or server/share"
        username.stringValue = saved?.username ?? ""
        username.placeholderString = "Username or DOMAIN\\username"
        password.placeholderString = saved == nil ? "Password" : "Saved in Keychain; enter to replace"
        for (title, y) in [("Type", 278.0), ("Server name, IP address or share URL", 220.0), ("Username", 164.0), ("Password", 108.0)] {
            let label = NSTextField(labelWithString: title)
            label.font = .systemFont(ofSize: 11); label.frame = NSRect(x: 0, y: y, width: 440, height: 16)
            view.addSubview(label)
        }
        guest.frame = NSRect(x: 0, y: 50, width: 230, height: 20)
        guest.state = saved?.guest == true ? .on : .off
        readOnly.frame = NSRect(x: 260, y: 50, width: 180, height: 20)
        readOnly.state = saved?.read_only == true ? .on : .off
        chooseFolder.frame = NSRect(x: 0, y: 26, width: 440, height: 20)
        remember.frame = NSRect(x: 0, y: 2, width: 440, height: 20); remember.state = .on
        for control in [protocolChoice, address, username, password, guest, readOnly, chooseFolder, remember] as [NSView] { view.addSubview(control) }
        address.nextKeyView = username; username.nextKeyView = password; password.nextKeyView = guest
    }
    func url() throws -> URL { try normalizedAddress(address.stringValue, scheme: protocolChoice.indexOfSelectedItem == 0 ? "smb" : "afp") }
}
@MainActor func editingMenu() {
    let menu = NSMenu(); let root = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
    let edit = NSMenu(title: "Edit")
    for (title, action, key) in [("Cut", "cut:", "x"), ("Copy", "copy:", "c"), ("Paste", "paste:", "v"), ("Select All", "selectAll:", "a")] {
        edit.addItem(NSMenuItem(title: title, action: Selector(action), keyEquivalent: key))
    }
    root.submenu = edit; menu.addItem(root); NSApp.mainMenu = menu
}
