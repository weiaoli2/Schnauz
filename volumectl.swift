// Read mounted local drives and their capacity. No directory traversal or
// recursive size calculation; capacity is supplied by the filesystem.
import Foundation

struct Volume: Encodable {
    let name: String
    let path: String
    let total: UInt64?
    let free: UInt64?
}

let fm = FileManager.default
let keys: Set<URLResourceKey> = [
    .volumeNameKey, .volumeIsLocalKey, .volumeTotalCapacityKey,
    .volumeAvailableCapacityKey, .volumeAvailableCapacityForImportantUsageKey
]
let identityKeys: Set<URLResourceKey> = [.volumeNameKey, .volumeIsLocalKey]
let urls = fm.mountedVolumeURLs(includingResourceValuesForKeys: Array(identityKeys), options: [.skipHiddenVolumes]) ?? []
var seen = Set<String>()
var volumes: [Volume] = []
// The sealed system root can be omitted by skipHiddenVolumes. Always include
// the startup drive explicitly, while filtering out its internal APFS mounts.
for url in [URL(fileURLWithPath: "/")] + urls {
    let path = url.path
    guard path == "/" || url.deletingLastPathComponent().path == "/Volumes",
          seen.insert(path).inserted,
          let info = try? url.resourceValues(forKeys: identityKeys), info.volumeIsLocal == true else { continue }
    // The system root is read-only on modern macOS; its paired Data volume
    // holds the user's files and gives the useful APFS space figure.
    let probe = path == "/" ? fm.homeDirectoryForCurrentUser : url
    let space = (try? probe.resourceValues(forKeys: keys)) ?? info
    let total = space.volumeTotalCapacity.flatMap { $0 > 0 ? UInt64($0) : nil }
    // Some macOS versions return zero for the important-usage key even while
    // regular free space is available. Never mistake that for a full drive.
    let available = [space.volumeAvailableCapacityForImportantUsage,
                     space.volumeAvailableCapacity.map(Int64.init)].compactMap { $0 }.max()
    let free = available.flatMap { $0 >= 0 ? UInt64($0) : nil }
    volumes.append(Volume(name: info.volumeName ?? url.lastPathComponent, path: path, total: total, free: free))
}
volumes.sort { $0.path == "/" || ($1.path != "/" && $0.name.localizedStandardCompare($1.name) == .orderedAscending) }
let data = try JSONEncoder().encode(volumes)
FileHandle.standardOutput.write(data)
