// Pure fixtures: no mounted-drive queries, NSWorkspace or real unmount/eject calls.
import Foundation
@main enum Tests {
    static func main() {
        precondition(canEject(path: "/Volumes/USB", local: true, internalDevice: false, removable: false, ejectable: false, identity: "disk-A"))
        precondition(!canEject(path: "/", local: true, internalDevice: false, removable: true, ejectable: true, identity: "boot"))
        precondition(!canEject(path: "/Volumes/SSD", local: true, internalDevice: true, removable: false, ejectable: false, identity: "internal"))
        precondition(!canEject(path: "/Volumes/NAS", local: false, internalDevice: false, removable: true, ejectable: true, identity: "network"))
        precondition(!canEject(path: "/Volumes/USB", local: true, internalDevice: false, removable: false, ejectable: false, identity: nil))
        let volume = Volume(name: "USB", path: "/Volumes/USB", local: true, total: 100, free: 50, identity: "disk-A", ejectable: true)
        var calls: [String] = []
        precondition(!ejectVolume(path: volume.path, identity: "disk-B", mounted: [volume], perform: { calls.append($0) }).success)
        precondition(!ejectVolume(path: "/", identity: "disk-A", mounted: [volume], perform: { calls.append($0) }).success)
        precondition(!ejectVolume(path: volume.path, identity: "disk-A", mounted: [], perform: { calls.append($0) }).success)
        precondition(calls.isEmpty)
        let busy = ejectVolume(path: volume.path, identity: "disk-A", mounted: [volume]) { _ in
            throw NSError(domain: "Fixture", code: 1, userInfo: [NSLocalizedDescriptionKey: "The disk is in use."])
        }
        precondition(!busy.success && busy.message.contains("in use"))
        let success = ejectVolume(path: volume.path, identity: "disk-A", mounted: [volume], perform: { calls.append($0) })
        precondition(success.success && calls == [volume.path])
        print("PASS external-drive eligibility, captured identity, removed drives, busy errors and explicit successful eject; fixtures only")
    }
}
