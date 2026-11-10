#!/usr/bin/env python3
"""Development-only conversion; shipped recognition runs ONNX in Rust."""
import argparse
import hashlib
import urllib.request
from pathlib import Path

REVISION = "c75f8e721f3eb86b3081ea8ddca81f2e333bc8e4"
SOURCE = f"https://media.githubusercontent.com/media/ShreyShingala/Pokemon-Card-Scanning-Webapp/{REVISION}/detector_models/pokemon_detector4/weights/best.pt"
SHA256 = "07d9fe5adcf302985600ad86b3f937361fc35878382917ae0dd0e990805cd7d8"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("directory", type=Path)
args = parser.parse_args()
args.directory.mkdir(parents=True, exist_ok=True)
checkpoint = args.directory / "best.pt"
if not checkpoint.exists():
    with urllib.request.urlopen(SOURCE, timeout=60) as response:
        content = response.read(7 * 1024 * 1024)
    if hashlib.sha256(content).hexdigest() != SHA256:
        raise SystemExit("Unexpected source checkpoint checksum")
    checkpoint.write_bytes(content)
if hashlib.sha256(checkpoint.read_bytes()).hexdigest() != SHA256:
    raise SystemExit("Unexpected source checkpoint checksum")
from ultralytics import YOLO
model = YOLO(str(checkpoint))
if model.names != {0: "pokemon_card"}:
    raise SystemExit("Unexpected detector classes")
output = Path(model.export(format="onnx", imgsz=640, opset=17, simplify=False, dynamic=False, device="cpu"))
print("Model:", output.resolve())
print("SHA-256:", hashlib.sha256(output.read_bytes()).hexdigest())
