import AppKit
struct FolderLabelResponse: Encodable {var cancelled: Bool; var label: FolderLabel?}
struct Response:Encodable {var cancelled:Bool;var link:WebLink?}
@main struct WeblinkMain {
    @MainActor static func main() {
        let input=FileHandle.standardInput.readDataToEndOfFile()
        if CommandLine.arguments.contains("--folder-label") {
            guard let label = try? JSONDecoder().decode(FolderLabel.self, from: input) else { exit(1) }
            let app = NSApplication.shared
            app.setActivationPolicy(.accessory)
            installEditingMenu()
            let result = editFolderLabel(label)
            if let data = try? JSONEncoder().encode(FolderLabelResponse(cancelled: result == nil, label: result)) {
                FileHandle.standardOutput.write(data)
            }
            return
        }
        guard let link=try? JSONDecoder().decode(WebLink.self,from:input) else {exit(1)}
        let app=NSApplication.shared
        app.setActivationPolicy(.accessory)
        installEditingMenu()
        let result=editWebLink(link)
        let response=Response(cancelled:result==nil,link:result)
        if let data=try? JSONEncoder().encode(response) {FileHandle.standardOutput.write(data)}
    }
}
