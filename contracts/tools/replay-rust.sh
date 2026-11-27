#!/usr/bin/env bash
# 对 Rust Runtime 回放同一请求，输出到独立目录，供 diff-contract.sh 比对。
set -euo pipefail

BASE_URL="${DSH_RUST_URL:-http://127.0.0.1:8090}"
SCENARIO=""
OUT=""
ENDPOINT="message"

usage() {
  cat <<'EOF'
用法:
  replay-rust.sh --scenario <dir> --out <dir> [--endpoint message|stream]

环境变量:
  DSH_RUST_URL  Rust Runtime 地址，默认 http://127.0.0.1:8090
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --scenario) SCENARIO="${2:-}"; shift 2 ;;
    --out) OUT="${2:-}"; shift 2 ;;
    --endpoint) ENDPOINT="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "未知参数: $1" >&2; usage; exit 2 ;;
  esac
done

[[ -n "$SCENARIO" && -n "$OUT" ]] || { usage; exit 2; }
REQUEST="$SCENARIO/request.json"
[[ -f "$REQUEST" ]] || { echo "缺少 $REQUEST" >&2; exit 2; }

mkdir -p "$OUT"

case "$ENDPOINT" in
  message)
    curl -fsS -X POST "$BASE_URL/api/agent/message" \
      -H 'Content-Type: application/json' \
      --data-binary @"$REQUEST" -o "$OUT/response.json"
    ;;
  stream)
    RAW="$OUT/stream.sse"
    curl -fsS -N -X POST "$BASE_URL/api/agent/stream" \
      -H 'Content-Type: application/json' \
      --data-binary @"$REQUEST" -o "$RAW"
    python3 - "$RAW" "$OUT/stream.ndjson" <<'PY'
import json
import sys

raw_path, out_path = sys.argv[1], sys.argv[2]
events = []
name = None
data = []


def flush():
    global name, data
    if name is None:
        return
    payload = "\n".join(data).strip()
    events.append({
        "seq": len(events) + 1,
        "event": name,
        "data": json.loads(payload) if payload else {},
    })
    name = None
    data = []


with open(raw_path, encoding="utf-8") as handle:
    for line in handle:
        line = line.rstrip("\n")
        if line.startswith(":"):
            continue
        if line.startswith("event:"):
            name = line[6:].strip()
        elif line.startswith("data:"):
            data.append(line[5:].strip())
        elif line == "":
            flush()
flush()

with open(out_path, "w", encoding="utf-8") as handle:
    for event in events:
        handle.write(json.dumps(event, ensure_ascii=False, separators=(",", ":")) + "\n")
print(f"normalized {len(events)} SSE events -> {out_path}")
PY
    ;;
  *)
    echo "未知 endpoint: $ENDPOINT" >&2
    exit 2
    ;;
esac

echo "输出目录: $OUT"