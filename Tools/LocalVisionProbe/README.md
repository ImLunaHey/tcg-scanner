# Local visual-assessment experiment

Development-only Swift probe. It loads local MLX model weights and reads saved
images. It does not contact ScannerService, control hardware, upload images, or
write grades into scan records. Outputs are unvalidated model text, never actions.
The application does not invoke this executable.

The initial evaluation failed the physical finish check described in
[the assessment report](../../Docs/local-visual-assessment.md). Do not use its
condition predictions as verified selling grades.

Prerequisites: Xcode with the Metal Toolchain (`xcodebuild -downloadComponent
MetalToolchain`), local model files, and Apple Silicon with sufficient memory.
No model weights or scan images are committed. Obtain model files from the pinned
source in the report; load only the local directory during inference.

From the repository root:

```sh
cd Tools/LocalVisionProbe
xcodebuild -scheme ScannerVisionProbe -destination 'platform=macOS' \
  -derivedDataPath ../../.build/local-vision-probe CODE_SIGNING_ALLOWED=NO build
../../.build/local-vision-probe/Build/Products/Debug/Probe \
  "$HOME/Library/Application Support/Scanner/models/qwen35-4b-4bit" \
  Prompts/assessment.txt /absolute/path/front.png /absolute/path/back.png
```

The program caps generated output at 400 tokens and resizes input to at most the
configured 1024 × 1024 processing target. Run one evaluation at a time. This is
an inspection experiment, not a throughput benchmark or an automated classifier.
Tokenization, image processing and generation run in Swift/MLX in this process;
no Python, Ollama or helper inference server is used.
