import AppKit

func expect(_ value: Bool) { assert(value) }

@main struct NetworkTests {
    @MainActor static func main() throws {
        _ = NSApplication.shared
        editingMenu()
        let server = try normalizedAddress("192.168.1.20", scheme: "smb")
        expect(server.absoluteString == "smb://192.168.1.20")
        let share = try normalizedAddress("SERVER/Shared Files", scheme: "smb")
        expect(share.host == "server" && share.path == "/Shared Files")
        let ipv6 = try normalizedAddress("smb://[::1]/Share", scheme: "smb")
        expect(ipv6.path == "/Share")
        for invalid in ["smb://user:secret@server/Share", "http://server/Share", "smb://", "smb://host/Share?password=secret", "host\n/share", "smb://host:0"] {
            do { _ = try normalizedAddress(invalid, scheme: "smb"); fatalError("Invalid address accepted") } catch NetworkFailure.address {}
        }
        expect(try relativePath("/Volumes/Share/素材", root: "/Volumes/Share") == "素材")
        for path in ["/Volumes/Share-1/file", "/Volumes/Share/../Other"] {
            do { _ = try relativePath(path, root: "/Volumes/Share"); fatalError("Outside folder accepted") } catch NetworkFailure.relative {}
        }
        for path in ["../outside", "/outside", "child/../../outside"] {
            do { _ = try resolvedPath(root: "/Volumes/Share", relative: path); fatalError("Escaping saved path accepted") } catch NetworkFailure.relative {}
        }
        expect(try resolvedPath(root: "/Volumes/Share-1", relative: "Folder/素材") == "/Volumes/Share-1/Folder/素材")
        expect(credentialAccount(url: share, username: "alice") != credentialAccount(url: share, username: "bob"))
        expect(credentialAccount(url: share, username: "alice") != credentialAccount(url: server, username: "alice"))
        let location = NetworkLocation(url: share.absoluteString, username: "alice", guest: false, read_only: true, mount_root: "/Volumes/Shared Files", relative: "Folder")
        let data = try JSONEncoder().encode(location)
        expect(!String(decoding: data, as: UTF8.self).contains("password"))
        expect(try JSONDecoder().decode(NetworkLocation.self, from: data) == location)
        let form = NetworkForm(location)
        expect(form.address.frame.width == 440)
        expect(form.address.frame.minY > form.username.frame.maxY && form.username.frame.minY > form.password.frame.maxY)
        expect(form.password.stringValue.isEmpty && form.readOnly.state == .on && form.remember.state == .on)
        expect(form.address.nextKeyView === form.username && form.username.nextKeyView === form.password)
        expect(NSApp.mainMenu?.items.first?.submenu?.items.contains(where: { $0.keyEquivalent == "v" }) == true)
        expect(try form.url() == share)
        let afp = NetworkForm(NetworkLocation(url: "afp://server/Share", username: "", guest: true, read_only: false, mount_root: "/Volumes/Share", relative: ""))
        expect(afp.protocolChoice.indexOfSelectedItem == 1 && afp.guest.state == .on)
        print("PASS network address/path validation, mount-name remapping, credential account isolation, nonsecret persistence and native form geometry; no windows, mounts, servers or Keychain accessed")
    }
}
