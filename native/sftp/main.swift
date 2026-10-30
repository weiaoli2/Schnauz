// Secrets use private stdin/stdout only. No argv, environment, files or logs.
import Foundation
let response: Response
var input = Data()
while input.count < 70000 {
    let chunk = FileHandle.standardInput.readData(ofLength: min(4096, 70000 - input.count))
    if chunk.isEmpty { break }
    input.append(chunk)
}
if input.count < 70000, let request = try? JSONDecoder().decode(Request.self, from: input) {
    response = handle(request, store: KeychainStore())
} else { response = Response(message: "Invalid credential request.") }
if let data = try? JSONEncoder().encode(response) { FileHandle.standardOutput.write(data) }
