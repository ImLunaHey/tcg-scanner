"""Replay camera_components fixtures through a running camera API, serially.

Development tooling only. Measures HTTP round trips, not phone capture/encoding.
Frames are ephemeral; the camera endpoint does not save scans.
"""
import argparse
import base64
import json
from pathlib import Path
import random
import time
import urllib.error
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("components", type=Path)
    parser.add_argument("origin", help="Running service origin, preferably its HTTPS Tailscale URL")
    parser.add_argument("output", type=Path)
    parser.add_argument("--rounds", type=int, default=3)
    args = parser.parse_args()
    fixtures = {row["id"]: row for row in json.loads(args.components.read_text())["rows"]}
    schedule = list(fixtures) * args.rounds
    random.Random(42).shuffle(schedule)
    rows = []
    for fixture_id in schedule:
        fixture = fixtures[fixture_id]
        body = json.dumps({"language": "en", "base64": base64.b64encode(
            Path(fixture["path"]).read_bytes()).decode()}).encode()
        request = urllib.request.Request(args.origin.rstrip("/") + "/api/v1/camera/identify",
                                         body, {"Content-Type": "application/json"})
        started = time.perf_counter()
        try:
            with urllib.request.urlopen(request, timeout=75) as response:
                result = json.load(response)
        except (urllib.error.URLError, TimeoutError) as error:
            result = {"state": "error", "error": str(error)}
        milliseconds = (time.perf_counter() - started) * 1000
        candidates = [candidate["id"] for candidate in result.get("candidates", [])]
        expected = fixture["expected"]
        outcome = ("correct" if result.get("state") == "matched" and candidates == [expected]
                   else "correctRejection" if expected is None and result.get("state") in ("cardBack", "noCard", "noMatch")
                   else "wrong" if result.get("state") == "matched"
                   else "unresolved")
        rows.append({"id": fixture_id, "expected": expected, "milliseconds": milliseconds,
                     "outcome": outcome, "result": result})
        args.output.write_text(json.dumps({"origin": args.origin, "rows": rows}, indent=2))
        print(f"{fixture_id}: {milliseconds:.0f} ms {outcome} ({result.get('state')})", flush=True)


if __name__ == "__main__":
    main()
