import AppKit

@MainActor func run(_ request: NetworkRequest) -> NetworkResponse {
    let form = NetworkForm(request.location)
    let alert = NSAlert()
    alert.messageText = "Connect to Network Drive"
    alert.informativeText = "Select the share in the macOS connection window. Schnauz opens it in a new tab."
    alert.accessoryView = form.view
    alert.addButton(withTitle: "Connect"); alert.addButton(withTitle: "Cancel")
    alert.window.initialFirstResponder = form.address
    var showForm = request.mode != "reconnect"
    while true {
        if showForm {
            NSApp.activate(ignoringOtherApps: true)
            guard alert.runModal() == .alertFirstButtonReturn else { return NetworkResponse(cancelled: true) }
        }
        do {
            let url = try form.url()
            let guest = form.guest.state == .on
            let username = form.username.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard guest || !username.isEmpty else { throw NetworkFailure.authentication }
            let typed = form.password.stringValue
            let password = guest ? nil : (typed.isEmpty ? try NetworkSystem.password(url, username) : typed)
            let mounts = try NetworkSystem.mount(url: url, username: username, password: password, guest: guest, readOnly: form.readOnly.state == .on)
            guard !mounts.isEmpty else { return NetworkResponse(cancelled: true) }
            var response = NetworkResponse()
            for (shareURL, root) in mounts {
                var relative = request.mode == "reconnect" ? request.location?.relative ?? "" : ""
                if form.chooseFolder.state == .on {
                    let picker = NSOpenPanel(); picker.canChooseDirectories = true; picker.canChooseFiles = false
                    picker.canCreateDirectories = false; picker.allowsMultipleSelection = false
                    picker.directoryURL = URL(fileURLWithPath: root); picker.prompt = "Open in Schnauz"
                    guard picker.runModal() == .OK, let selected = picker.url else { return NetworkResponse(cancelled: true) }
                    relative = try relativePath(selected.path, root: root)
                }
                let path = try resolvedPath(root: root, relative: relative)
                if !guest, let password, form.remember.state == .on {
                    do { try NetworkSystem.savePassword(password, url: shareURL, username: username) }
                    catch { response.warning = "Connected, but the password could not be saved in Schnauz's Keychain. Reconnecting may ask for it again." }
                }
                response.folders.append(ConnectedFolder(location: NetworkLocation(url: shareURL.absoluteString, username: username, guest: guest, read_only: form.readOnly.state == .on, mount_root: root, relative: relative), path: path))
            }
            return response
        } catch {
            alert.informativeText = (error as? LocalizedError)?.errorDescription ?? "Could not connect. Please try again."
            showForm = true
        }
    }
}
@main struct NetworkMain {
    @MainActor static func main() {
        let input = FileHandle.standardInput.readDataToEndOfFile()
        if let request = try? JSONDecoder().decode(NetworkRequest.self, from: input) {
            NSApplication.shared.setActivationPolicy(.accessory)
            editingMenu()
            let response = run(request)
            if let data = try? JSONEncoder().encode(response) { FileHandle.standardOutput.write(data) }
        }
    }
}
