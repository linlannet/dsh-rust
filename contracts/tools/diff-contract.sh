#!/usr/bin/env bash
# 归一化后比较两份契约输出。支持 .json 与 .ndjson。
set -euo pipefail

EXPECTED="${1:-}"
ACTUAL="${2:-}"
IGNORE="${DIFF_IGNORE:-timestamp traceId durationMs capturedAt createdAt updatedAt}"

if [[ -z "$EXPECTED" || -z "$ACTUAL" ]]; then
  echo "用法: diff-contract.sh <expected.json|.ndjson> <actual.json|.ndjson>" >&2
  echo "环境变量 DIFF_IGNORE 可追加忽略字段（空格分隔）" >&2
  exit 2
fi
[[ -f "$EXPECTED" ]] || { echo "不存在: $EXPECTED" >&2; exit 2; }
[[ -f "$ACTUAL" ]] || { echo "不存在: $ACTUAL" >&2; exit 2; }

python3 - "$EXPECTED" "$ACTUAL" "$IGNORE" <<'PY'
import difflib
import json
import sys

expected_path, actual_path, ignore_raw = sys.argv[1:4]
ignore = set(ignore_raw.split())


def load(path):
    with open(path, encoding="utf-8") as handle:
        text = handle.read().strip()
    if path.endswith(".ndjson"):
        return [json.loads(line) for line in text.splitlines() if line.strip()]
    return json.loads(text)


def norm(value):
    if isinstance(value, dict):
        return {k: norm(v) for k, v in value.items() if k not in ignore}
    if isinstance(value, list):
        return [norm(v) for v in value]
    return value


left = json.dumps(norm(load(expected_path)), ensure_ascii=False, indent=2, sort_keys=True).splitlines()
right = json.dumps(norm(load(actual_path)), ensure_ascii=False, indent=2, sort_keys=True).splitlines()
diff = list(difflib.unified_diff(left, right, fromfile=expected_path, tofile=actual_path, lineterm=""))
if diff:
    print("\n".join(diff))
    sys.exit(1)
print("contract match")
PY