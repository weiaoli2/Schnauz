// Mounted-drive metadata and explicit native Eject. No traversal or forced unmount.
import Foundation
import AppKit
import Darwin

func mountIdentity(_ url: URL, persistent: String?) -> String? {
    // Add the kernel filesystem identity, including on filesystems without UUIDs.
    let descriptor = url.withUnsafeFileSystemRepresentation { path -> Int32 in
        guard let path else { return -1 }
        return Darwin.open(path, O_EVTONLY | O_CLOEXEC)
    }
    guard descriptor >= 0 else { return nil }
    defer { Darwin.close(descriptor) }
    var filesystem = statfs()
    guard fstatfs(descriptor, &filesystem) == 0 else { return nil }
    return "uuid:\(persistent ?? "")|fsid:\(filesystem.f_fsid.val.0):\(filesystem.f_fsid.val.1)"
}

func mountedVolumes() -> [Volume] {
    let fm = FileManager.default
    let keys: Set<URLResourceKey> = [
        .volumeNameKey, .volumeIsLocalKey, .volumeTotalCapacityKey,
        .volumeAvailableCapacityKey, .volumeAvailableCapacityForImportantUsageKey
    ]
    let identityKeys: Set<URLResourceKey> = [
        .volumeNameKey, .volumeIsLocalKey, .volumeUUIDStringKey,
        .volumeIsInternalKey, .volumeIsRemovableKey, .volumeIsEjectableKey
    ]
    let urls = fm.mountedVolumeURLs(includingResourceValuesForKeys: Array(identityKeys), options: [.skipHiddenVolumes]) ?? []
    var seen = Set<String>()
    var volumes: [Volume] = []
    for url in [URL(fileURLWithPath: "/")] + urls {
        let path = url.path
        guard path == "/" || url.deletingLastPathComponent().path == "/Volumes",
              seen.insert(path).inserted,
              let info = try? url.resourceValues(forKeys: identityKeys) else { continue }
        // APFS Data supplies useful capacity for the sealed startup volume.
        let probe = path == "/" ? fm.homeDirectoryForCurrentUser : url
        let local = info.volumeIsLocal == true
        let space = local ? ((try? probe.resourceValues(forKeys: keys)) ?? info) : info
        let total = space.volumeTotalCapacity.flatMap { $0 > 0 ? UInt64($0) : nil }
        let available = [space.volumeAvailableCapacityForImportantUsage,
                         space.volumeAvailableCapacity.map(Int64.init)].compactMap { $0 }.max()
        let free = available.flatMap { $0 >= 0 ? UInt64($0) : nil }
        let identity = local ? mountIdentity(url, persistent: info.volumeUUIDString) : nil
        let ejectable = canEject(path: path, local: local, internalDevice: info.volumeIsInternal,
                                  removable: info.volumeIsRemovable == true, ejectable: info.volumeIsEjectable == true,
                                  identity: identity)
        volumes.append(Volume(name: info.volumeName ?? url.lastPathComponent, path: path, local: local,
                              total: total, free: free, identity: identity, ejectable: ejectable))
    }
    volumes.sort { $0.path == "/" || ($1.path != "/" && $0.name.localizedStandardCompare($1.name) == .orderedAscending) }
    return volumes
}

@main enum VolumeControl {
    @MainActor static func main() throws {
        let args = CommandLine.arguments
        let data: Data
        if args.count == 1 {
            data = try JSONEncoder().encode(mountedVolumes())
        } else if args.count == 4 && args[1] == "--eject" {
            // Re-resolve identity at click execution; opening a menu never ejects.
            let result = ejectVolume(path: args[2], identity: args[3], mounted: mountedVolumes()) { path in
                try NSWorkspace.shared.unmountAndEjectDevice(at: URL(fileURLWithPath: path))
            }
            data = try JSONEncoder().encode(result)
        } else {
            data = try JSONEncoder().encode(EjectResponse(success: false, message: "Invalid volume request."))
        }
        FileHandle.standardOutput.write(data)
    }
}
