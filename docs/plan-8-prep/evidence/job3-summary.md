# JOB 3 — Java `freerouting` MCP stdio bridge, observed behaviour

Evidence: `job3-mcp.jsonl` (30 records: run A = protocol/error probes, run B = pipeline with API auth ON,
run C = pipeline with API auth OFF). Raw stdout lines are verbatim; nothing in `tools/list` is truncated.
Jar: `/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar`
(`serverVersion` = `2.3.1-SNAPSHOT`). Scratch: `plan8-evidence/job3/` (drivers, stderr logs, routed .ses).

## 1. Launch command (verbatim)

Run A (protocol + error probes):

```
/opt/homebrew/opt/openjdk@25/bin/java \
  -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
  -jar /Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar \
  --api_server.enabled=true \
  --mcp_server.enabled=true \
  --mcp_server.stdio=true \
  --user_data_path=<scratch>/job3/userdata
```

Run C (adds the two flags needed to reach the stateful job tools):

```
  --api_server.enabled=true --api_server.authentication.enabled=false \
  --mcp_server.authentication.enabled=false \
  --mcp_server.enabled=true --mcp_server.stdio=true
```

Facts about the flags (`Freerouting.java` ~1437, `McpServerSettings`, `ApiServerSettings`):
- `--mcp_server.stdio=true` **alone is not enough**. `McpServerSettings.isEnabled` defaults to `false`,
  and the bridge is only started inside `if (globalSettings.mcpServerSettings.isEnabled)`. So
  `--mcp_server.enabled=true` is mandatory.
- `--api_server.enabled=true` is also effectively mandatory for every *generated* tool:
  `ApiServerSettings.isEnabled` defaults to `false` too, and generated tools are HTTP calls to
  `mcp_server.target_api_base_url` (default `http://127.0.0.1:37864`, auto-rewritten to the API
  server's real local port when the API server starts). Without it, generated tools fail at connect.
- Ports: MCP Jetty `http://127.0.0.1:37964`, API Jetty `http://127.0.0.1:37864` (defaults). Both were
  free; no port blocker encountered. stderr line 5 of run A:
  `MCP server started successfully at http://127.0.0.1:37964. JSON-RPC endpoint: .../v1/mcp,
   SSE: .../v1/mcp/events, WebSocket: .../v1/mcp/ws.`
- `mcp_server.stdio` in `freerouting.json` is deliberately IGNORED (warning at `Freerouting.java:1187`);
  only the CLI arg / `FREEROUTING__MCP_SERVER__STDIO` env var select stdio mode.
- In stdio mode `System.setOut(System.err)`, so all normal logging (including a full
  `[mcp][cid=…] request=…` / `response=…` echo of every message) goes to stderr.

## 2. initialize — verbatim result

Raw stdout line (record n=1):

```
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","serverName":"Freerouting MCP","serverVersion":"2.3.1-SNAPSHOT","serverInfo":{"name":"Freerouting MCP","version":"2.3.1-SNAPSHOT"},"capabilities":{"tools":{}}}}
```

- `protocolVersion` = `"2024-11-05"` (hard-coded string).
- `serverInfo` = `{"name":"Freerouting MCP","version":"<Constants.FREEROUTING_VERSION>"}`.
- `capabilities` = `{"tools":{}}` — no `resources`, no `prompts`, no `logging`, no `listChanged`.
- **Non-spec extra top-level keys in `result`: `serverName` and `serverVersion`** (duplicates of
  `serverInfo.name`/`.version`). Key order is: protocolVersion, serverName, serverVersion, serverInfo,
  capabilities.
- `initialize` with `params:{}` (no `clientInfo`) succeeds identically (record n=12); `clientInfo` is
  only harvested into a static `detectedClientInfo` used later as the `Freerouting-Environment-Host`
  header and analytics tag (default `"MCP-Client/1.0"` — this leaks into created sessions: run C's
  `create_session` body came back with `"host": "MCP-Client/1.0"`).

## 3. notifications/initialized

- The controller computes `{"jsonrpc":"2.0","id":null,"result":{}}` (visible in stderr) but never sends
  it: `isNotification` (id absent or null) makes the endpoint return **HTTP 204 No Content**.
- The bridge sees a non-null empty body and therefore prints **an empty line** to stdout, then flushes.
  Record n=2 `raw_stdout_line` is `""`.
- So the bridge does NOT block on notifications, but it does emit a stray blank line per notification.
  A line-oriented client that expects "no output for a notification" will desynchronise.

## 4. tools/list — 28 tools

Full raw schemas are in `job3-mcp.jsonl` record n=3 (phase `tools_list`). Summary
(`name` — input-schema top-level buckets; every bucket object is `additionalProperties:false`):

24 generated (Swagger scan of `/v1/*`), three-bucket wrapper `{path,query,body}` — only the buckets
that the operation actually needs are emitted, and each present bucket is listed in the top-level
`required`:

| tool | input buckets |
|---|---|
| `cancel_job` | `path{jobId}` |
| `create_session` | (none — `properties:{}`) |
| `download_job_output_file` | `path{jobId}` |
| `download_job_output_json` | `path{jobId}` |
| `enqueue_job` | `body{id,created_at,short_name,name,started_at,finished_at,state,stage,priority,session_id,input,output,rules,drc,router_settings,drc_settings,resource_usage,current_pass,inputFileDetails,outputFileDetails,cancelledByUser,inputFromFile,rulesFile,eagleScriptFile,dummyInputFile,settings,duration}` |
| `get_job_details` | `path{jobId}` |
| `get_job_drc_report` | `path{jobId}` |
| `get_job_logs` | `path{jobId}` |
| `get_session_details` | (none) — description says "List all sessions" (name/description mismatch) |
| `get_session_logs` | `path{sessionId}` |
| `get_system_environment` | (none) |
| `get_system_status` | (none) |
| `identify_user` | `body{userId,anonymousId,context,event,traits,properties}` |
| `list_jobs` | `path{sessionId}` |
| `monitor_session` | `path{sessionId}` |
| `post_v1_jobs_jobid_rules` | `path{jobId}`, `body{size,crc32,format,statistics,filename,directoryPath,job_id,data,…}` |
| `start_job` | `path{jobId}` |
| `stream_job_logs` | `path{jobId}` |
| `stream_job_output_file` | `path{jobId}` |
| `stream_job_output_json` | `path{jobId}` |
| `track_user_action` | `body{userId,anonymousId,context,event,traits,properties}` |
| `update_job_settings` | `path{jobId}`, `body{enabled,algorithm,fanout,copperToEdgeClearanceUm,…,traceCosts}` |
| `upload_job_input_file` | `path{jobId}`, `body{size,crc32,format,statistics,filename,directoryPath,job_id,data,…}` |
| `upload_job_input_json` | `path{jobId}`, `body:string` |

4 hand-written "custom" tools — **flat arguments, NOT the three-bucket wrapper**:

| tool | input |
|---|---|
| `encode_base64` | `text:string` (required) |
| `decode_base64` | `base64:string` (required) |
| `upload_job_input_from_local_file` | `jobId:string`, `filePath:string` (both required) |
| `download_job_output_to_local_file` | `jobId:string`, `filePath:string` (both required) |

Notes:
- No `query` bucket appeared on any tool in this build — the wrapper is `path`/`body` in practice,
  though `arguments.query` is still read by `invokeTool` and is accepted when supplied.
- Every tool carries an `outputSchema` (28/28). The 24 generated ones share
  `{status:integer, contentType:string, body:<any, "HTTP response body, parsed as JSON when possible">}`
  with `required:["status","contentType","body"]`, and the per-operation OpenAPI response description is
  glued on as the schema's `description`.
- `post_v1_jobs_jobid_rules` is the fallback auto-name for an operation with no `operationId`
  (`<method>_<path with / and {} flattened>`, lower-cased). A port copying names must reproduce that.

### Worked example — the three-bucket wrapper (record n=…, phase `authOff_p2_start_job`)

```json
{"jsonrpc":"2.0","id":106,"method":"tools/call",
 "params":{"name":"start_job","arguments":{"path":{"jobId":"db89f795-1ec2-4643-876d-e9f88c6e66f0"}}}}
```

Path params are substituted into the raw OpenAPI path (`URLEncoder`, `+` → `%20`); `query` entries are
appended as query params (values via `getAsString()`); `body` is serialised with Gson and sent only for
POST/PUT/PATCH (missing body ⇒ literal `{}`). Omitting the buckets entirely on a no-arg tool works
(`get_system_status` with `arguments:{}` and with `{"path":{},"query":{},"body":{}}` both return 200) —
the schema's `additionalProperties:false` is **not enforced** at call time.

## 5. tools/call result shape

Happy path, verbatim stdout (record n=4):

```
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\n  \"status\": 200,\n  \"contentType\": \"application/json\",\n  \"body\": {\n    \"base64\": \"aGVsbG8gd29ybGQ=\"\n  }\n}"}],"isError":false}}
```

- `result` = `{"content":[{"type":"text","text":"<pretty-printed JSON envelope>"}], "isError":<bool>}`.
- **Exactly one text block**; the `{status, contentType, body}` envelope is *stringified* into it by
  `GsonProvider.GSON.toJson(...)`, which pretty-prints — so the text contains `\n` and two-space indents.
- **No `structuredContent` key at all**, despite every tool declaring an `outputSchema`.
- `isError` = `status >= 400` (HTTP status of the proxied REST call). Protocol-level failures
  (unknown tool, missing `name`) are JSON-RPC `error` objects instead, with no `result`/`isError`.
- Custom tools use the same envelope: `encode_base64` → `body:{base64:…}`, `decode_base64` →
  `body:{text:…}`, the two local-file tools → `body:{message:"Successfully …: <path>"}`.

## 6. Error paths (all observed; process stayed alive through all of them)

| trigger | raw response |
|---|---|
| `tools/call` unknown tool `no_such_tool_xyz` | `{"jsonrpc":"2.0","id":6,"error":{"code":-32601,"message":"Unknown tool: no_such_tool_xyz"}}` |
| unknown method `ping` | `{"jsonrpc":"2.0","id":7,"error":{"code":-32601,"message":"Unknown method: ping"}}` — **confirmed: `ping` is NOT implemented** |
| malformed JSON line `{"jsonrpc":"2.0","id":8,"method":"tools/list"` | `{  "jsonrpc": "2.0",  "error": {    "code": -32700,    "message": "Invalid JSON: java.io.EOFException: End of input at line 1 column 46 path $.method"  }}` |
| request with no `id` (`tools/list`) | HTTP 204 ⇒ bridge prints an **empty line**; no JSON at all |
| `tools/call` with no `params.name` | `{"jsonrpc":"2.0","id":9,"error":{"code":-32602,"message":"Missing required parameter: name"}}` |
| REST 401 through a generated tool (auth ON) | `result` with `isError:true` and envelope `{"status":401,…,"body":{"error":"Missing API key. Please provide a valid API key in the Authorization header using Bearer scheme (Authorization: Bearer <API_KEY>). You can apply for a free API key at https://www.freerouting.app."}}` |

No `data` member is ever present on an error object — `error()` only writes `code` and `message`.
Other codes reachable from source but not triggered here: `-32602 "Authentication failed: …"` (no
profile header), `-32602 "Missing required parameter: text|base64|jobId|filePath"`,
`-32601 "Unknown custom tool: …"`, `-32603 "Internal error"`, `-32603 "Failed to upload/download …"`.

### The -32700 line is the odd one out
Every other response is returned as `Response.ok(response.toString())` — a compact JSON *string*.
The `-32700` branch returns the `JsonObject` itself, so Jersey/Gson serialises it **pretty-printed**;
the bridge then strips `\r` and `\n`, leaving the collapsed-but-still-double-spaced line above.
It also has **no `id` member** (Gson drops the JSON-null id), unlike every other error which carries
`"id":<the id>`.

## 7. EOF / exit

Closing the child's stdin ended all three runs: bridge logs
`MCP stdio bridge detected EOF, shutting down application.` on stderr and calls `System.exit(0)`.
Observed process exit code **0** in runs A, B and C. `ps` and `lsof` afterwards: no JVM, ports 37864/37964
free. (A read failure on stdin would `System.exit(1)`; not reachable from a normal close.)

## 8. Full job pipeline over stdio MCP (run C)

Completed end-to-end on `fixtures/Issue143-rpi_splitter.dsn`:
`create_session` → 200 `{id, user_id, host:"MCP-Client/1.0"}` →
`enqueue_job` with `body:{session_id, name}` → 200 job `{id, short_name:"DB89F7", state:"QUEUED"…}` →
`upload_job_input_from_local_file{jobId, filePath}` → 200 `{message:"Successfully uploaded input from file: …"}` →
`start_job{path:{jobId}}` → 200 `state:"READY_TO_START"` → one poll of `get_job_details` 3 s later →
`state:"COMPLETED"` → `download_job_output_to_local_file{jobId, filePath}` → 200, wrote a 3654-byte
Specctra `.ses` to disk → `download_job_output_file{path:{jobId}}` → 200 with `{job_id, data:<base64 SES>}`.
All pairs are in `job3-mcp.jsonl` under the `authOff_` phase prefix.

Run B (identical, but with the API server's default authentication) stalled immediately: `create_session`
and `enqueue_job` both returned envelope `status:401` / `isError:true` with the "Missing API key" message.

## 9. UNDOCUMENTED / SURPRISING (things the survey did not predict)

- **`--mcp_server.stdio=true` is not sufficient.** Both `mcp_server.enabled` and (for every generated
  tool) `api_server.enabled` default to `false`; the practical minimum is three flags.
- **The stateful half of the tool surface is unreachable out of the box.** `ApiAuthenticationSettings.
  isEnabled` defaults to **true**, and the stdio bridge never supplies an `Authorization` header —
  it only forwards one that arrived on the MCP request, which over stdio there is none. So a bare
  `--mcp_server.enabled=true --mcp_server.stdio=true --api_server.enabled=true` run gets HTTP 401 on
  every session/job tool. `--api_server.authentication.enabled=false` is required (or a real API key).
  Only the unauthenticated `/v1/system/*` endpoints work otherwise.
- **Notifications produce a blank stdout line**, not silence: 204 + non-null empty body ⇒
  `originalOut.println("")`. Same for any request missing `id`. Line-oriented clients must tolerate it.
- **Newline stripping is destructive, not cosmetic.** The bridge does
  `body.replace("\r","").replace("\n","")` — no re-escaping. It is safe only because valid JSON never
  contains raw newlines *inside* strings, but it visibly mangles the one pretty-printed response
  (`-32700`) into double-spaced JSON. Note the *inner* tool text is pretty-printed too, but its newlines
  are already `\n` escape sequences inside a JSON string, so they survive.
- **`-32700` responses have no `id` field at all** and different whitespace from every other response.
- **The tool registry is rebuilt on every single request.** `OpenApiMcpToolRegistry.fromApplication()`
  runs a fresh Swagger `JaxrsOpenApiContextBuilder(...).buildContext(true).read()` scan of package
  `app.freerouting.api` for *each* `tools/list` **and each `tools/call`** (the call path re-scans just to
  resolve one tool name). No cache. First `tools/list` in run A took ~2 s of wall time.
- **28 tools, not ~29**: 24 generated + 4 custom, in this jar.
- **`outputSchema` is declared but never honoured.** No `structuredContent` is ever returned, and for the
  4 custom tools the declared `outputSchema` (`{base64}` / `{text}` / `{message}`) does not even match
  what they actually return (the `{status,contentType,body}` envelope wrapping those).
- **Custom tools break the wrapper convention**: flat `text` / `base64` / `jobId` / `filePath` arguments
  while all 24 generated tools use `{path,query,body}`. Two argument conventions coexist in one list.
- **The three-bucket wrapper is advisory.** `additionalProperties:false` and `required` are published but
  not validated; unknown/omitted buckets are silently ignored (`getObject` returns an empty object).
- **`query` bucket is vestigial in this build** — no tool declares one, though the caller may pass one.
- **Auto-naming leaks HTTP shape** into the tool list for operations without an `operationId`:
  `post_v1_jobs_jobid_rules`.
- **`clientInfo` from `initialize` is stored in a `static volatile` field** and reused as the
  `Freerouting-Environment-Host` for later REST calls of *any* connection; the default `"MCP-Client/1.0"`
  ends up persisted as the created session's `host`.
- **The MCP server also exposes SSE (`/v1/mcp/events`) and a WebSocket (`/v1/mcp/ws`)**, and every
  `tools/call` broadcasts an `mcp.tool.called` event on that bridge. Not part of the stdio protocol,
  but it is running.
- **stderr echoes every request and response in full** (`[mcp][cid=…] request=…` / `response=…`),
  including any file contents passed as base64 — a privacy/noise consideration.
- `X-Internal-Bridge-Token` is sent by the bridge on every POST, but nothing in the MCP controller path
  checks it in this build; auth is the `Freerouting-Profile-ID` header, which the bridge always supplies
  (default `00000000-0000-0000-0000-000000000000` when no profile/env var is set).
