#!/usr/bin/env bash
# 从 Java 基线录制一个场景。需要已启动的 dsh-java 与假模型/假工具。
set -euo pipefail

CONTRACTS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE_URL="${DSH_JAVA_URL:-http://127.0.0.1:8090}"
SCENARIO=""
ENDPOINT="message"
REQUEST=""

usage() {
  cat <<'EOF'
用法:
  record-from-java.sh --scenario <dir> --request <file> [--endpoint message|stream]

环境变量:
  DSH_JAVA_URL  Java 服务地址，默认 http://127.0.0.1:8090
  DSH_JAVA_DIR  dsh-java 源码目录，用于记录 commit
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --scenario) SCENARIO="${2:-}"; shift 2 ;;
    --endpoint) ENDPOINT="${2:-}"; shift 2 ;;
    --request) REQUEST="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "未知参数: $1" >&2; usage; exit 2 ;;
  esac
done

[[ -n "$SCENARIO" && -n "$REQUEST" ]] || { usage; exit 2; }
[[ -f "$REQUEST" ]] || { echo "请求文件不存在: $REQUEST" >&2; exit 2; }

mkdir -p "$SCENARIO"
cp "$REQUEST" "$SCENARIO/request.json"

case "$ENDPOINT" in
  message)
    curl -fsS -X POST "$BASE_URL/api/agent/message" \
      -H 'Content-Type: application/json' \
      --data-binary @"$REQUEST" -o "$SCENARIO/response.json"
    ;;
  stream)
    RAW="$SCENARIO/stream.sse"
    curl -fsS -N -X POST "$BASE_URL/api/agent/stream" \
      -H 'Content-Type: application/json' \
      --data-binary @"$REQUEST" -o "$RAW"
    python3 - "$RAW" "$SCENARIO/stream.ndjson" <<'PY'
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

python3 - "$SCENARIO" "$ENDPOINT" "${DSH_JAVA_DIR:-}" "$CONTRACTS_ROOT" <<'PY'
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

scenario, endpoint, java_dir, root = sys.argv[1:5]
meta_path = os.path.join(scenario, "meta.json")
old = {}
if os.path.exists(meta_path):
    try:
        with open(meta_path, encoding="utf-8") as handle:
            old = json.load(handle)
    except Exception:
        old = {}

commit = None
repo = None
if java_dir:
    repo = "dsh-java"
    try:
        commit = subprocess.check_output(
            ["git", "-C", java_dir, "rev-parse", "HEAD"], text=True
        ).strip()
    except Exception:
        commit = None

meta = {
    "scenario": os.path.relpath(scenario, os.path.join(root, "fixtures")).replace(os.sep, "/"),
    "contracts": old.get("contracts", {}),
    "origin": "recorded",
    "recorded": True,
    "capturedAt": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "source": {
        "repo": repo,
        "commit": commit,
        "endpoint": "POST /api/agent/" + endpoint,
    },
    "normalized": old.get("normalized", ["timestamp", "id", "traceId", "durationMs"]),
    "redacted": old.get("redacted", ["apiKey", "authorization"]),
}
if not meta["contracts"]:
    print("提示: meta.contracts 为空，请补充对应 schema 路径后再校验。")

with open(meta_path, "w", encoding="utf-8") as handle:
    json.dump(meta, handle, ensure_ascii=False, indent=2)
print(f"wrote {meta_path}")
PY

echo "请检查脱敏与归一化后再提交。"