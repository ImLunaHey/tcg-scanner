import Foundation
import Darwin
import MLXVLM
import MLXLMCommon
import Tokenizers

@main struct Probe {
    static func main() async {
        do { try await run() }
        catch {
            FileHandle.standardError.write(Data("Local vision probe failed: \(error.localizedDescription)\n".utf8))
            exit(1)
        }
    }
    private static func run() async throws {
        guard (4...5).contains(CommandLine.arguments.count) else {
            FileHandle.standardError.write(Data("Usage: Probe MODEL_DIRECTORY PROMPT_FILE FRONT_IMAGE [BACK_IMAGE]\n".utf8))
            exit(64)
        }
        let folder = URL(fileURLWithPath: CommandLine.arguments[1])
        let model = try await loadModelContainer(from: folder, using: LocalTokenizerLoader())
        let session = ChatSession(model, generateParameters: .init(maxTokens: 400, temperature: 0), processing: .init(resize: CGSize(width: 1024, height: 1024)), additionalContext: ["enable_thinking": false])
        let prompt = try String(contentsOfFile: CommandLine.arguments[2], encoding: .utf8)
        let paths = Array(CommandLine.arguments.dropFirst(3))
        let answer = try await session.respond(to: prompt, images: makeImages(paths), videos: [], audios: [])
        print(answer)
    }
}

nonisolated func makeImages(_ paths: [String]) -> sending [UserInput.Image] { paths.map { .url(URL(fileURLWithPath: $0)) } }
