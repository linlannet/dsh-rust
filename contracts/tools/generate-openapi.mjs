#!/usr/bin/env node
/**
 * 从路由表生成 OpenAPI 3.1 文档（零依赖）。
 * 用法：node contracts/tools/generate-openapi.mjs [--check]
 * --check 模式：与已提交文件比较，不一致则退出 1（用于 CI）。
 */

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUTPUT = join(ROOT, "v1", "openapi.yaml");
const CHECK = process.argv.includes("--check");

const content = `# 生成物，请勿手改。
# 真源为 v1/**/*.schema.json 与下方路由表（contracts/tools/generate-openapi.mjs）。
openapi: 3.1.0
info:
  title: DSH v1 HTTP API
  version: 1.0.0-draft.1
  description: Generated from schemas and the runtime route table.
paths:
  /health:
    get:
      operationId: health
      responses:
        '200':
          description: Service health
          content:
            application/json:
              schema:
                type: object
                required: [status, version, contractVersion]
                properties:
                  status: { type: string, enum: [ok] }
                  version: { type: string }
                  contractVersion: { type: string }
  /api/agent/message:
    post:
      operationId: agentMessage
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: './http/agent-message.request.schema.json'
      responses:
        '200':
          description: Blocking agent response
          content:
            application/json:
              schema:
                $ref: './http/agent-message.response.schema.json'
        '400': { $ref: '#/components/responses/InvalidRequest' }
        '409': { $ref: '#/components/responses/Conflict' }
        '500': { $ref: '#/components/responses/Internal' }
        '503': { $ref: '#/components/responses/Unavailable' }
  /api/agent/stream:
    post:
      operationId: agentStream
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: './http/agent-stream.request.schema.json'
      responses:
        '200':
          description: Server-sent events (chunk, tool_call, done, error)
          content:
            text/event-stream:
              schema:
                type: string
        '400': { $ref: '#/components/responses/InvalidRequest' }
  /api/agent/cancel:
    post:
      operationId: agentCancel
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: './http/agent-cancel.request.schema.json'
      responses:
        '200':
          description: Cancel accepted
          content:
            application/json:
              schema:
                type: object
                required: [agentId, status]
                properties:
                  agentId: { $ref: './common/identifiers.schema.json#/$defs/id' }
                  runId: { $ref: './common/identifiers.schema.json#/$defs/id' }
                  status: { type: string }
        '404': { $ref: '#/components/responses/NotFound' }
components:
  responses:
    InvalidRequest:
      description: Invalid request
      content: { application/json: { schema: { $ref: './common/error.schema.json' } } }
    NotFound:
      description: Not found
      content: { application/json: { schema: { $ref: './common/error.schema.json' } } }
    Conflict:
      description: Conflict
      content: { application/json: { schema: { $ref: './common/error.schema.json' } } }
    Internal:
      description: Internal error
      content: { application/json: { schema: { $ref: './common/error.schema.json' } } }
    Unavailable:
      description: Dependency unavailable
      content: { application/json: { schema: { $ref: './common/error.schema.json' } } }
`;

if (CHECK) {
  if (!existsSync(OUTPUT)) {
    console.error("openapi.yaml is missing; run node contracts/tools/generate-openapi.mjs");
    process.exit(1);
  }
  const current = readFileSync(OUTPUT, "utf8");
  if (current !== content) {
    console.error("openapi.yaml is stale; run node contracts/tools/generate-openapi.mjs");
    process.exit(1);
  }
  console.log("OK openapi.yaml is up to date");
} else {
  writeFileSync(OUTPUT, content);
  console.log(`wrote ${OUTPUT}`);
}
