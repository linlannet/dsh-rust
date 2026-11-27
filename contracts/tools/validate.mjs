#!/usr/bin/env node
/**
 * contracts 零依赖校验器。
 * - 校验 schema 元信息、$id 唯一性与 $ref 可解析性
 * - 校验 HTTP、SSE、plugin-host、persistence fixture
 * - 校验 SSE 顺序与终态
 */

import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { dirname, join, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const V1 = join(ROOT, "v1");
const FIX = join(ROOT, "fixtures");

const errors = [];
const warnings = [];
const stats = { schemas: 0, refs: 0, fixtures: 0 };

const rel = (p) => relative(ROOT, p).split(sep).join("/");
const fail = (msg) => errors.push(msg);
const warn = (msg) => warnings.push(msg);

function walk(dir) {
  if (!existsSync(dir)) return [];
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else out.push(p);
  }
  return out;
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    fail(`${rel(file)}: JSON 解析失败: ${error.message}`);
    return null;
  }
}

const schemas = new Map();
const ids = new Map();

for (const file of walk(V1).filter((p) => p.endsWith(".schema.json"))) {
  const schema = readJson(file);
  if (!schema) continue;
  stats.schemas += 1;
  schemas.set(file, schema);

  if (schema.$schema !== "https://json-schema.org/draft/2020-12/schema") {
    fail(`${rel(file)}: $schema 必须为 Draft 2020-12`);
  }
  if (typeof schema.title !== "string" || !schema.title) {
    fail(`${rel(file)}: 缺少 title`);
  }
  if (typeof schema.$id !== "string" || !schema.$id.startsWith("dsh.v1.")) {
    fail(`${rel(file)}: $id 需以 dsh.v1. 开头`);
  } else if (ids.has(schema.$id)) {
    fail(`${rel(file)}: $id 重复 ${schema.$id}（另见 ${rel(ids.get(schema.$id))}）`);
  } else {
    ids.set(schema.$id, file);
  }
}

function pointerNode(schema, pointer) {
  let cur = schema;
  for (const raw of pointer.split("/").slice(1)) {
    const key = raw.replaceAll("~1", "/").replaceAll("~0", "~");
    if (cur && typeof cur === "object" && key in cur) cur = cur[key];
    else return undefined;
  }
  return cur;
}

function resolveRef(ref, fromFile) {
  if (typeof ref !== "string") return null;
  if (/^https?:\/\//.test(ref)) {
    fail(`${rel(fromFile)}: 禁止绝对 $ref ${ref}`);
    return null;
  }

  const [filePart, pointer = ""] = ref.split("#");
  let targetFile = fromFile;
  let target = schemas.get(fromFile);

  if (filePart) {
    targetFile = resolve(dirname(fromFile), filePart);
    if (!existsSync(targetFile)) {
      fail(`${rel(fromFile)}: $ref 文件不存在 ${filePart}`);
      return null;
    }
    target = schemas.get(targetFile);
    if (!target) {
      target = readJson(targetFile);
      if (target) schemas.set(targetFile, target);
    }
    if (!target) return null;
  }

  stats.refs += 1;
  const node = pointer ? pointerNode(target, pointer) : target;
  if (node === undefined) {
    fail(`${rel(fromFile)}: $ref 指针不存在 ${ref}`);
    return null;
  }
  return { schema: node, file: targetFile };
}

function scanRefs(node, file) {
  if (Array.isArray(node)) {
    for (const item of node) scanRefs(item, file);
    return;
  }
  if (!node || typeof node !== "object") return;
  if (typeof node.$ref === "string") resolveRef(node.$ref, file);
  for (const value of Object.values(node)) scanRefs(value, file);
}

for (const [file, schema] of schemas) scanRefs(schema, file);

function typeMatches(value, type) {
  switch (type) {
    case "object":
      return value !== null && typeof value === "object" && !Array.isArray(value);
    case "array":
      return Array.isArray(value);
    case "integer":
      return Number.isInteger(value);
    case "number":
      return typeof value === "number" && Number.isFinite(value);
    case "string":
      return typeof value === "string";
    case "boolean":
      return typeof value === "boolean";
    case "null":
      return value === null;
    default:
      return true;
  }
}

function check(instance, schema, path, file, depth = 0) {
  if (depth > 40) return [`${path}: $ref 嵌套过深`];
  if (schema === true) return [];
  if (schema === false) return [`${path}: 不允许出现该值`];
  if (!schema || typeof schema !== "object") return [];

  if (schema.$ref) {
    const resolved = resolveRef(schema.$ref, file);
    if (!resolved) return [`${path}: 无法解析 $ref ${schema.$ref}`];
    return check(instance, resolved.schema, path, resolved.file, depth + 1);
  }

  const out = [];

  if (schema.type) {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    if (!types.some((type) => typeMatches(instance, type))) {
      out.push(`${path}: 期望 ${types.join("|")}，实际 ${Array.isArray(instance) ? "array" : instance === null ? "null" : typeof instance}`);
    }
  }

  if ("const" in schema && instance !== schema.const) {
    out.push(`${path}: 期望常量 ${JSON.stringify(schema.const)}`);
  }

  if (Array.isArray(schema.enum) && !schema.enum.some((value) => value === instance)) {
    out.push(`${path}: 值 ${JSON.stringify(instance)} 不在枚举 [${schema.enum.join(", ")}]`);
  }

  if (typeof instance === "string") {
    if (typeof schema.minLength === "number" && instance.length < schema.minLength) {
      out.push(`${path}: 长度小于 ${schema.minLength}`);
    }
    if (typeof schema.pattern === "string" && !new RegExp(schema.pattern).test(instance)) {
      out.push(`${path}: 不匹配 ${schema.pattern}`);
    }
    if (schema.format === "date-time" && !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(instance)) {
      out.push(`${path}: 不是 UTC ISO-8601 时间`);
    }
  }

  if (typeof instance === "number") {
    if (typeof schema.minimum === "number" && instance < schema.minimum) {
      out.push(`${path}: 小于最小值 ${schema.minimum}`);
    }
    if (typeof schema.maximum === "number" && instance > schema.maximum) {
      out.push(`${path}: 大于最大值 ${schema.maximum}`);
    }
  }

  if (Array.isArray(instance)) {
    if (typeof schema.minItems === "number" && instance.length < schema.minItems) {
      out.push(`${path}: 元素少于 ${schema.minItems}`);
    }
    if (schema.items) {
      instance.forEach((item, index) => {
        out.push(...check(item, schema.items, `${path}[${index}]`, file, depth + 1));
      });
    }
  }

  if (typeMatches(instance, "object")) {
    for (const key of schema.required ?? []) {
      if (!(key in instance)) out.push(`${path}: 缺少必填字段 ${key}`);
    }
    const props = schema.properties ?? {};
    for (const [key, value] of Object.entries(instance)) {
      if (key in props) {
        out.push(...check(value, props[key], `${path}.${key}`, file, depth + 1));
      } else if (schema.additionalProperties === false) {
        out.push(`${path}: 不允许的字段 ${key}`);
      } else if (schema.additionalProperties && typeof schema.additionalProperties === "object") {
        out.push(...check(value, schema.additionalProperties, `${path}.${key}`, file, depth + 1));
      }
    }
  }

  for (const sub of schema.allOf ?? []) {
    out.push(...check(instance, sub, path, file, depth + 1));
  }

  if (Array.isArray(schema.anyOf)) {
    const passed = schema.anyOf.some((sub) => check(instance, sub, path, file, depth + 1).length === 0);
    if (!passed) out.push(`${path}: 不满足 anyOf`);
  }

  if (Array.isArray(schema.oneOf)) {
    const passed = schema.oneOf.filter((sub) => check(instance, sub, path, file, depth + 1).length === 0).length;
    if (passed !== 1) out.push(`${path}: 期望 oneOf 恰好 1 个分支匹配，实际 ${passed}`);
  }

  return out;
}

function loadSchema(relPath, fromFile) {
  const file = resolve(ROOT, relPath);
  if (!existsSync(file)) {
    fail(`${rel(fromFile)}: 契约文件不存在 ${relPath}`);
    return null;
  }
  let schema = schemas.get(file);
  if (!schema) {
    schema = readJson(file);
    if (schema) schemas.set(file, schema);
  }
  return schema ? { schema, file } : null;
}

function validateFile(file, contract) {
  const value = readJson(file);
  if (value === null) return;
  for (const issue of check(value, contract.schema, rel(file), contract.file)) fail(issue);
}

function metaFiles(dir) {
  return walk(dir).filter((p) => p.endsWith(`${sep}meta.json`)).map((p) => dirname(p));
}

function requireMeta(meta, file, keys) {
  if (!meta) return false;
  for (const key of keys) {
    if (!(key in meta)) fail(`${rel(file)}: meta 缺少 ${key}`);
  }
  return true;
}

// HTTP
for (const dir of metaFiles(join(FIX, "http"))) {
  stats.fixtures += 1;
  const metaFile = join(dir, "meta.json");
  const meta = readJson(metaFile);
  if (!requireMeta(meta, metaFile, ["scenario", "contracts", "origin", "recorded"])) continue;
  const contracts = meta.contracts ?? {};
  if (!contracts.request || !contracts.response) {
    fail(`${rel(metaFile)}: contracts 需要 request 与 response`);
    continue;
  }
  const requestSchema = loadSchema(contracts.request, metaFile);
  const responseSchema = loadSchema(contracts.response, metaFile);
  if (requestSchema) validateFile(join(dir, "request.json"), requestSchema);
  if (responseSchema) validateFile(join(dir, "response.json"), responseSchema);
}

// SSE
const SSE_SCHEMAS = {
  chunk: "v1/sse/chunk.schema.json",
  tool_call: "v1/sse/tool-call.schema.json",
  done: "v1/sse/done.schema.json",
  error: "v1/sse/error.schema.json"
};

for (const dir of metaFiles(join(FIX, "sse"))) {
  stats.fixtures += 1;
  const metaFile = join(dir, "meta.json");
  const meta = readJson(metaFile);
  if (!requireMeta(meta, metaFile, ["scenario", "contracts", "origin", "recorded"])) continue;
  const envelope = loadSchema(meta.contracts?.envelope ?? "v1/sse/envelope.schema.json", metaFile);
  if (!envelope) continue;

  const streamFile = join(dir, "stream.ndjson");
  if (!existsSync(streamFile)) {
    fail(`${rel(dir)}: 缺少 stream.ndjson`);
    continue;
  }
  const lines = readFileSync(streamFile, "utf8").split("\n").filter((line) => line.trim());
  let terminal = 0;
  lines.forEach((line, index) => {
    const where = `${rel(streamFile)}:${index + 1}`;
    let event;
    try {
      event = JSON.parse(line);
    } catch (error) {
      fail(`${where}: JSON 解析失败: ${error.message}`);
      return;
    }
    for (const issue of check(event, envelope.schema, where, envelope.file)) fail(issue);
    if (event.seq !== index + 1) fail(`${where}: seq 应为 ${index + 1}，实际 ${event.seq}`);
    if (event.event === "done" || event.event === "error") terminal += 1;
    if (terminal > 0 && index < lines.length - 1) {
      fail(`${where}: 终态事件之后仍有事件`);
    }
    const schemaPath = SSE_SCHEMAS[event.event];
    if (!schemaPath) {
      warn(`${where}: 未知事件 ${event.event}，按开放枚举处理`);
      return;
    }
    const dataSchema = loadSchema(schemaPath, metaFile);
    if (dataSchema) {
      for (const issue of check(event.data, dataSchema.schema, `${where}.data`, dataSchema.file)) fail(issue);
    }
  });
  if (terminal !== 1) fail(`${rel(streamFile)}: 期望恰好 1 个终态事件，实际 ${terminal}`);
}

// plugin-host
for (const dir of metaFiles(join(FIX, "plugin-host"))) {
  stats.fixtures += 1;
  const metaFile = join(dir, "meta.json");
  const meta = readJson(metaFile);
  if (!requireMeta(meta, metaFile, ["scenario", "contracts", "origin", "recorded"])) continue;
  const envelope = loadSchema(meta.contracts?.envelope ?? "v1/plugin-host/envelope.schema.json", metaFile);
  if (!envelope) continue;
  const transcript = join(dir, "transcript.ndjson");
  if (!existsSync(transcript)) {
    fail(`${rel(dir)}: 缺少 transcript.ndjson`);
    continue;
  }
  readFileSync(transcript, "utf8")
    .split("\n")
    .filter((line) => line.trim())
    .forEach((line, index) => {
      const where = `${rel(transcript)}:${index + 1}`;
      let frame;
      try {
        frame = JSON.parse(line);
      } catch (error) {
        fail(`${where}: JSON 解析失败: ${error.message}`);
        return;
      }
      for (const issue of check(frame, envelope.schema, where, envelope.file)) fail(issue);
    });
}

// persistence
for (const dir of metaFiles(join(FIX, "persistence"))) {
  stats.fixtures += 1;
  const metaFile = join(dir, "meta.json");
  const meta = readJson(metaFile);
  if (!requireMeta(meta, metaFile, ["scenario", "contracts", "origin", "recorded"])) continue;
  const contract = loadSchema(meta.contracts?.event ?? "v1/persistence/session-event.schema.json", metaFile);
  if (!contract) continue;
  const eventsFile = join(dir, "events.ndjson");
  if (!existsSync(eventsFile)) {
    fail(`${rel(dir)}: 缺少 events.ndjson`);
    continue;
  }
  readFileSync(eventsFile, "utf8")
    .split("\n")
    .filter((line) => line.trim())
    .forEach((line, index) => {
      const where = `${rel(eventsFile)}:${index + 1}`;
      let event;
      try {
        event = JSON.parse(line);
      } catch (error) {
        fail(`${where}: JSON 解析失败: ${error.message}`);
        return;
      }
      for (const issue of check(event, contract.schema, where, contract.file)) fail(issue);
    });
}

// index
const indexPath = join(FIX, "index.json");
if (existsSync(indexPath)) {
  const index = readJson(indexPath);
  const seen = new Set();
  for (const item of index?.cases ?? []) {
    if (seen.has(item.id)) fail(`${rel(indexPath)}: 场景 id 重复 ${item.id}`);
    seen.add(item.id);
    if (!existsSync(join(FIX, item.path))) fail(`${rel(indexPath)}: 场景目录不存在 ${item.path}`);
  }
}

for (const message of warnings) console.warn(`WARN ${message}`);
if (errors.length) {
  console.error(`FAIL ${errors.length} 个问题`);
  for (const message of errors) console.error(`  - ${message}`);
  process.exit(1);
}
console.log(`OK schemas=${stats.schemas} refs=${stats.refs} fixtures=${stats.fixtures} warnings=${warnings.length}`);
