---
name: portal-api
description: Answer questions and take action on a user's StreamingFast Portal account — billing, subscription, usage, live load, and the full hosted-deployment lifecycle — by calling the Portal API directly. Billing/usage routes are read-only; the hosted-deployment routes cover both reads (list/status/events/logs) and mutations (deploy, scale replicas, undeploy, reset, reconfigure). Use when the user asks things like "what's our current plan", "what was our usage last month", "who's connected right now", "is my sink running", "list my deployments", "deploy this spkg", "scale my sink to 3 replicas", or "undeploy X". The skill tells the assistant how to authenticate, which endpoint to call, when to confirm destructive actions, and how to summarize the result in plain language.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.5.0
  author: StreamingFast
  documentation: https://docs.substreams.dev
---

# StreamingFast Portal API — Conversational Billing, Usage & Hosted-Deployment Skill

This skill lets the assistant act on an organization's StreamingFast Portal account by calling the Portal API directly — answering billing/usage/load questions **and** managing the full hosted-deployment lifecycle (deploy, scale, undeploy, reset, reconfigure). The user is **not** expected to write code against this API. The assistant is.

Two safety tiers:

- **Read routes** (all billing/usage/load + deployment list/status/events/logs) — call freely and summarize.
- **Mutating routes** (deploy, set replica, undeploy, reset, update/delete config, deploy database, cleanup) — these change live infrastructure and may incur cost or destroy data. **Confirm with the user before calling**, echoing back exactly what will happen. Treat `ResetDeployment` (drops/truncates the database), `Undeploy`, `DeleteDeploymentConfig`, and `CleanupOrphanedDeployment` as destructive and irreversible. See "Mutations — Confirmation Protocol" below.

## When This Skill Applies

Trigger on natural-language questions about portal state, for example:

- "What's our current subscription / plan / tier?"
- "What's our usage this month / last month / this year?"
- "How much have we used on API key `key_...`?"
- "Which network / service is driving most of our usage?"
- "What will our next bill look like?"
- "Who's connected right now? / how many workers are in use?"
- "Should we upgrade or downgrade based on our usage?"
- "Is my sink / deployment running? / what's its head block and lag?"
- "List my hosted deployments / what's deployed?"
- "What happened to deployment `xxx`? / show its recent events / show its logs."
- "Deploy this spkg as a SQL sink / foundational store."
- "Scale my sink to N replicas / pause it (set replicas to 0)."
- "Undeploy / reset / reconfigure deployment `xxx`."

If the user asks about anything outside billing / subscription / usage / live-load / hosted deployments, this skill is not the right one — defer. Read questions are answered directly; mutating actions require explicit confirmation first (see "Mutations — Confirmation Protocol").

## First-Run Setup

`BASE_URL` — default `https://admin.streamingfast.io`; if the user says "local" / "localhost" → `http://localhost:9000`; another host → use as-is. Assume the default; do not ask.

Authentication is the device-code login flow documented in the [`portal-api-jwt`](../portal-api-jwt/SKILL.md) skill. Run it whenever you don't already hold a valid access token. It does **not** yield a static credential — it yields a **session**. Hold these **in memory** for the session; do not write to disk uninvited:

| Item | What it is | Lifecycle |
|---|---|---|
| `access_token` | Bearer token sent as `Authorization: Bearer <access_token>` on every call. | Short-lived (~15 min). Expires mid-session — must be refreshed. |
| `refresh_token` | Renews the access token without re-prompting the user. | **Rotated on every use** — each refresh returns a new one; store it and discard the old. ~8 h absolute cap. |
| `ORG_ID` | The org is **fixed at login** (the user picks it during approval). | Don't ask for it separately; every call's `organization_id` must equal it. |

### Managing the session

Cache `access_token`, `refresh_token`, and the pinned `ORG_ID` in memory for the session. Then:

- **Refresh on expiry, don't re-prompt.** When the access token nears expiry *or* any call returns `unauthenticated`, call `RefreshToken` (per `portal-api-jwt` Step 5) to get a new access token, then retry the call.
- **Always replace the stored refresh token** with the one the refresh returns. **Never reuse a rotated refresh token** — replaying an old one revokes the whole token family and forces a fresh login.
- **When refresh fails** (refresh token hit its ~8 h cap, was revoked, or the user lost org access), discard both tokens and re-run the device-code login from Step 1.
- See `portal-api-jwt` for the full flow, lifetimes, and rotation rules.

### Starting a login

If an unexpired access token is already available (prior context or a prior `portal-api-jwt` login), use it — don't re-ask. Otherwise tell the user you'll log them in, then run the `portal-api-jwt` flow:

> "To do that I'll connect to your StreamingFast Portal account. I'll start an interactive browser login — you'll get a link and a short code to approve, and you'll choose which organization to grant access to. Ready when you are."

Never echo the access token or refresh token back in full; redact to the last 4 characters.

## How the Assistant Makes a Call

Every Portal API method is a `POST` of a JSON body to `{BASE_URL}/{Service}/{Method}`, where `{Service}` is `sf.portalapi.v1.PortalApi` for the six billing/usage routes and `sf.portalapi.v1.HostedService` for the hosted-deployment routes. The assistant invokes via the Bash tool with `curl`:

```bash
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.PortalApi/<METHOD>" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '<JSON_BODY>'

# Hosted-deployment routes live under a different service path:
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.HostedService/<METHOD>" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '<JSON_BODY>'
```

Pipe through `jq` to extract only the fields needed for the answer. Never dump raw JSON at the user unless they explicitly ask.

`$ACCESS_TOKEN` is the Bearer token from the [`portal-api-jwt`](../portal-api-jwt/SKILL.md) login (see First-Run Setup). If a call returns `unauthenticated`, the token has likely expired — refresh it per that skill and retry; don't re-prompt the user.

JSON wire form follows proto3 JSON mapping in **requests** (prefer **snake_case** field names). **Responses often use camelCase** (`deploymentId`, `organizationId`, `accessToken`, `exists`, `planName`, `substreamsConfig`, …). When parsing, accept **both** casings. Enum values are full strings (e.g. `"DATE_FILTER_CURRENT_MONTH"`, `"DEVICE_TOKEN_STATUS_APPROVED"`).

**Wire quirks agents hit in production:**

- **Zero values are omitted.** Proto3 JSON drops default/zero fields. Missing `plan_tier` often means `PLAN_TIER_COMMUNITY` (0); missing `total_cents` / `borrowed_workers` / `active_requests` often means `0`, not “unknown.” Treat absent numeric fields as zero when summarizing.
- **`int64` / `uint64` may arrive as JSON strings** (`"maxWorkers": "5"`, `"egressBytesQuota": "5368709120"`). Coerce with `Number(...)` / `int(...)` before math or formatting.
- **Empty objects are valid.** `GetBillingDetails` may return `{}` when the org has no company profile filled in; a scaled-down deployment may have `"state": {}` on `ListDeployments` / empty `deploymentState` on `GetDeploymentState` even when `success: true`.
- **Unknown request fields are silently ignored — they do NOT error.** The server decodes with `DiscardUnknown`, so a misspelled or removed field (e.g. `storage_class`, `sink_config`, `replicas` instead of `replica`) is dropped and the call still returns `200` / `success: true`. A typo therefore surfaces as *the setting didn't take effect*, never as `invalid_argument`. Spell field names exactly as the proto above; when a deployed config doesn't match what you sent, suspect a dropped field first. Enum fields are still validated — an invalid enum *value* does error.
- **Requests accept snake_case or camelCase**, but **responses are camelCase only** (no snake_case duplicates). Prefer snake_case in requests; always read camelCase in responses.

## The Endpoints — Inline Proto

Six billing/usage routes on `PortalApi` (read-only) plus the full `HostedService` surface (reads + mutations). Only the messages used by these routes are documented here. Unrelated `PortalApi` RPCs (key/org/payment management) remain out of scope.

### Service

```proto
service PortalApi {
  rpc GetOrganizationSubscription(GetOrganizationSubscriptionRequest)
      returns (Subscription);
  rpc GetBillingDetails(GetBillingDetailsRequest)
      returns (OrganizationBillingDetails);
  rpc GetUsageBilling(GetUsageBasedBillingRequest)
      returns (BillingPreview);
  rpc MultiServiceUsageSummaryByOrganization(MultiServiceUsageSummaryByOrganizationRequest)
      returns (MultiServiceUserConsumption);
  rpc UsageByOrganization(UsageRequestByOrganization)
      returns (UserUsage);
  rpc ActiveConnections(ActiveConnectionsRequest)
      returns (ActiveConnectionsResponse);
}

// Hosted deployments — entire service is agent-callable. Reads are gated by a
// view check, mutations by a manage (OWNER/ADMIN) check, server-side.
service HostedService {
  // Reads (call freely)
  rpc ListDeployments(ListDeploymentsRequest)       returns (ListDeploymentsResponse);
  rpc GetDeployment(GetDeploymentRequest)           returns (GetDeploymentResponse);
  rpc GetDeploymentState(DeploymentStateRequest)    returns (DeploymentStateResponse);
  rpc GetDeploymentEvents(GetDeploymentEventsRequest) returns (GetDeploymentEventsResponse);
  rpc Logs(LogsRequest)                             returns (LogsResponse);
  rpc HasDeploymentSecret(HasDeploymentSecretRequest) returns (HasDeploymentSecretResponse); // existence check only — never returns the value

  // Mutations (CONFIRM FIRST)
  rpc CreateDeployment(CreateDeploymentRequest)     returns (CreateDeploymentResponse); // mint a unique deployment_id
  rpc Deploy(DeploymentRequest)                     returns (DeploymentResult);
  rpc DeployDatabase(DeployDatabaseRequest)         returns (DeployDatabaseResponse);
  // StoreDeploymentSecret stores the DB password server-side. It is NOT
  // agent-callable: the user enters the password in the Portal web UI so it
  // never travels through an agent request. The agent only references it.
  // rpc StoreDeploymentSecret(StoreDeploymentSecretRequest) returns (StoreDeploymentSecretResponse);
  rpc SetReplica(ReplicaRequest)                    returns (ReplicaResponse);
  rpc Undeploy(UndeployRequest)                     returns (UndeployResponse);          // destructive
  rpc ResetDeployment(ResetDeploymentRequest)       returns (ResetDeploymentResponse);   // destructive (drops/truncates DB)
  rpc UpdateDeploymentConfig(UpdateDeploymentConfigRequest) returns (UpdateDeploymentConfigResponse);
  rpc DeleteDeploymentConfig(DeleteDeploymentConfigRequest) returns (DeleteDeploymentConfigResponse); // destructive
  rpc CleanupOrphanedDeployment(CleanupOrphanedDeploymentRequest) returns (CleanupOrphanedDeploymentResponse); // destructive
}
```

### Request messages

```proto
message GetOrganizationSubscriptionRequest         { string organization_id = 1; }
message GetBillingDetailsRequest                   { string organization_id = 1; }
message MultiServiceUsageSummaryByOrganizationRequest { string organization_id = 1; }
message ActiveConnectionsRequest                   { string organization_id = 1; }

message GetUsageBasedBillingRequest {
  string organization_id = 1;
  MultiServiceOrganizationConsumption usage = 2;   // pass {} for current consumption
}

message UsageRequestByOrganization {
  Filter filter = 1;
  string organization_id = 2;
}

message Filter {
  DateFilter date_filter = 1;
  Provider   provider    = 2;
  Network    network     = 3;
  optional string api_key_id = 4;
  Service    service     = 5;
}
```

### Hosted-deployment request messages — reads

```proto
message ListDeploymentsRequest    { string organization_id = 1; }
message GetDeploymentRequest      { string deployment_id = 1; string organization_id = 2; }
message DeploymentStateRequest    { string deployment_id = 1; string organization_id = 2; }

message GetDeploymentEventsRequest {
  string deployment_id = 1;
  int32  limit         = 2;  // optional, default 100
  string organization_id = 3;
}

message LogsRequest {
  string deployment_id = 1;
  string pod_name      = 2;  // optional; empty = all pods
  int32  tail_lines    = 3;  // optional, default 100
  bool   follow        = 4;  // not implemented in v1 — leave false
  bool   previous      = 5;  // logs from the previous (crashed) container instance
  string organization_id = 6;
}

// HasDeploymentSecret — existence check only (never the value). Call it ONCE after
// the user confirms they saved the password — do NOT background-poll in a loop.
// Key is namespaced by output type: "postgres_password" | "clickhouse_password".
message HasDeploymentSecretRequest {
  string deployment_id = 1;
  string organization_id = 2;
  string key = 3;          // "postgres_password" | "clickhouse_password"
}
message HasDeploymentSecretResponse { bool exists = 1; }
```

`deployment_id` comes from a prior `ListDeployments` call. `organization_id` is the same org ID used for the billing routes.

### Hosted-deployment request messages — mutations (confirm first)

```proto
// CreateDeployment mints a fresh, unique deployment_id (server-generated —
// typically a UUID; do not assume a fixed "dep" prefix). Call it FIRST and reuse
// the returned id for the secret page, HasDeploymentSecret (after user
// confirmation), and Deploy. Never invent the id.
message CreateDeploymentRequest  { string organization_id = 1; }
message CreateDeploymentResponse { string deployment_id = 1; }

// Deploy creates a new hosted deployment. deployment_request carries the spkg,
// execution config, and (for sink-sql) the output database. See the
// sf.hosted.common.v1 payload types below.
message DeploymentRequest {
  string deployment_id = 1;                          // from CreateDeployment — do not invent it
  string name          = 2;
  sf.hosted.common.v1.DeploymentRequest deployment_request = 3;
  reserved 4;                                         // was api_key_id; the sink's key is auto-created server-side per deployment — do not send one
  string organization_id = 5;
  bool   use_stored_secret = 6;                      // resolve the output-DB password from the staged secret (vault) and deploy with an empty password — never send one inline; required for agent tokens
}
message DeploymentResult {}                            // empty on success

// SetReplica — scale up/down; count 0 = pause.
// NOTE: this request type is sf.hosted.common.v1.ReplicaRequest — it lives in the
// hosted module, not sf.portalapi.v1 (its response, ReplicaResponse, IS portalapi).
// The JSON body is unaffected; the fields are exactly as below.
message ReplicaRequest {   // package: sf.hosted.common.v1
  string deployment_id = 1;
  int64  count         = 2;
  string organization_id = 3;
}

message UndeployRequest          { string deployment_id = 1; string organization_id = 2; }

message ResetDeploymentRequest {
  string deployment_id = 1;
  bool   drop_schema   = 2;       // true = drop schema; false = truncate tables
  string organization_id = 3;
}

message UpdateDeploymentConfigRequest {
  string deployment_id = 1;
  sf.hosted.common.v1.SubstreamsPackage spkg = 2;
  sf.hosted.common.v1.ExecutionConfig execution_config = 3;
  bool   restart_from_scratch = 4;  // delete PVCs to reset cursor (foundational-store only)
  sf.hosted.common.v1.OutputConfig output_config = 5;
  string api_key_id    = 6;         // empty = leave bound key unchanged
  string organization_id = 7;
}

message DeleteDeploymentConfigRequest    { string deployment_id = 1; string organization_id = 2; }
message CleanupOrphanedDeploymentRequest { string deployment_id = 1; string organization_id = 2; }

message DeployDatabaseRequest {        // attach/validate the user's existing Postgres
                                       // connection — does NOT provision a database.
  string deployment_id = 1;
  sf.hosted.common.v1.Postgres postgres_spec = 2; // Postgres only; there is no clickhouse_spec.
                                       // For ClickHouse, put connection fields in Deploy's
                                       // outputConfig.clickhouse instead.
  string organization_id = 3;
  bool   use_stored_secret = 4;        // PREFERRED: ignore postgres_spec.password and
                                       // resolve it server-side from the deployment's
                                       // stored secret (entered by the user in the web UI).
                                       // Send the other postgres_spec fields (server, user,
                                       // database, schema, sslmode) but leave password empty.
}
```

### Imported `sf.hosted.common.v1` payload types (for Deploy / UpdateDeploymentConfig)

```proto
message DeploymentRequest {            // note: distinct from the portalapi wrapper above
  oneof deployment {
    SinkSqlDeployment           sink_sql_deployment           = 2;
    FoundationalStoreDeployment foundational_store_deployment = 3;
  }
}

message SinkSqlDeployment {
  SubstreamsPackage spkg            = 1;
  OutputConfig      outputConfig    = 60;  // note camelCase in JSON: "outputConfig"
  ExecutionConfig   execution_config = 3;
  int64             replica         = 2;
  string            network         = 4;   // e.g. "solana-mainnet", "ethereum-mainnet"
}

message FoundationalStoreDeployment {
  SubstreamsPackage       spkg            = 1;
  FoundationalStoreConfig config          = 2;
  ExecutionConfig         execution_config = 3;
  int64                   replica         = 4;
}

message FoundationalStoreConfig {
  string type_url              = 2;   // e.g. "sf.substreams.solana.spl.v1.AccountOwner"
  string network               = 3;
  bool   no_time_traversal     = 7;
  string token                 = 10;  // substreams API token
  string substreams_parameters = 11;
  FoundationalStoreMode mode   = 12;
  reserved 9;                         // was storage_class — removed upstream
}

enum FoundationalStoreMode {
  FOUNDATIONAL_STORE_MODE_UNSPECIFIED = 0;  // server default (Substreams)
  FOUNDATIONAL_STORE_MODE_SUBSTREAMS  = 1;
  FOUNDATIONAL_STORE_MODE_REMOTE_FEED = 2;
}
// There is no `storage_class` and no `resources` / ResourceConfig on this message —
// both were removed upstream. Pod resources and storage class are NOT
// agent-configurable; the runner applies its defaults. Sending either field is
// silently dropped (see "Wire quirks"), so the deployment will come up with default
// resources and nobody will be told. If a user needs custom CPU/memory/storage,
// say it can't be set through this API and point them at StreamingFast support.

message SubstreamsPackage {
  oneof source {
    bytes  binary            = 1;   // avoid for hosted agents — prefer URL
    string url               = 2;   // PREFERRED — publicly fetchable HTTPS
    string substreams_dev_id = 3;   // package on substreams.dev (secondary; see note)
  }
}
// Hosted Deploy: spkg MUST be a public internet URL the runner can GET without auth.
// PREFERRED after substreams publish:
//   url = "https://api.substreams.dev/v1/packages/<package-name>/<version>"
// substreams_dev_id ("name@v0.1.0") can fail on some sink runners (mis-parsed as
// "substreams-dev://name@version"). Prefer url. Local paths / private repos fail.

message ExecutionConfig {
  int64  start_block        = 1;
  int64  stop_block         = 2;
  string output_module      = 3;
  string filters            = 4;
  string parameters         = 5;
  string module_output_type = 6;  // "proto:sf.substreams.sink.database.v1.DatabaseChanges"
                                  // selects standard DatabaseChanges mode; any other
                                  // value runs sink-sql in from-proto mode
}

message OutputConfig {
  oneof config {
    Postgres              postgres                = 1;
    Clickhouse            clickhouse              = 2;
    GoogleCloudSqlPrivate google_cloud_sql_private = 3;
  }
}

message Postgres   { string server = 1; uint64 port = 2; string user = 3; string password = 4; string database = 5; string schema = 6; string sslmode = 7; }
message Clickhouse { string server = 1; uint64 port = 2; string user = 3; string password = 4; string database = 5; bool secure = 6; }
message GoogleCloudSqlPrivate { string instance_connection_name = 1; uint64 port = 2; string user = 3; string password = 4; string database = 5; string schema = 6; string credentials_secret_name = 7; }
```

`password` fields are secrets. **Do not ask the user to paste a database password into the chat, and never put one in a request you compose** — anything you receive lands in the transcript. Instead route the password through the stored-secret flow (see "Provide the database password" below): the user enters it once in the Portal web UI, and you deploy with `use_stored_secret: true` and an empty `password`. If a user volunteers a password anyway, decline to embed it, tell them why, and point them at the secure page. Never echo a password back, and never log the full request body.

### Snapshot input for `GetUsageBilling`

```proto
message MultiServiceOrganizationConsumption {
  uint64 firehose_egress_bytes        = 1;
  uint64 substreams_egress_bytes      = 2;
  uint64 substreams_processed_blocks  = 3;
  uint64 token_api_requests_total     = 4;
  uint64 firehose_processed_blocks    = 5;
  uint64 token_api_requests_token     = 6;
  uint64 token_api_requests_dex       = 7;
  uint64 token_api_requests_nft       = 8;
  uint64 token_api_requests_historical = 9;
}
```

Pass `{}` to preview against current consumption. Populate only when previewing a hypothetical bill.

### Enums

```proto
enum DateFilter {
  DATE_FILTER_UNSET         = 0;
  DATE_FILTER_LAST_7_DAYS   = 1;
  DATE_FILTER_LAST_MONTH    = 2;
  DATE_FILTER_CURRENT_MONTH = 3;
  DATE_FILTER_YESTERDAY     = 4;
  DATE_FILTER_LAST_30_DAYS  = 5;
  DATE_FILTER_TODAY         = 6;
  DATE_FILTER_CURRENT_YEAR  = 7;
  DATE_FILTER_LAST_YEAR     = 8;
}

enum Provider {
  PROVIDER_UNSET = 0;
  PROVIDER_ALL   = 1;
  PROVIDER_SF    = 2;
  PROVIDER_PINAX = 3;
}

enum Service {
  SERVICE_UNSET      = 0;
  SERVICE_SUBSTREAMS = 1;
  SERVICE_FIREHOSE   = 2;
  SERVICE_TOKEN_API  = 3;
}

enum ServiceType {
  SERVICE_TYPE_UNSET      = 0;
  SERVICE_TYPE_SUBSTREAMS = 1;
  SERVICE_TYPE_TOKEN_API  = 2;   // Firehose is bundled into the Substreams subscription
}

enum OverageType {
  OVERAGE_TYPE_UNSPECIFIED = 0;
  OVERAGE_TYPE_STRICT      = 1;  // hard cap — calls fail when quota exhausted
  OVERAGE_TYPE_METERED     = 2;  // pay-as-you-go past quota
}

enum PlanTier {
  PLAN_TIER_COMMUNITY  = 0;
  PLAN_TIER_SCALING    = 8;
  PLAN_TIER_METERED    = 10; // deprecated: use SCALING or PRO
  PLAN_TIER_PRO        = 12;
  PLAN_TIER_ENTERPRISE = 20;
  PLAN_TIER_CUSTOM     = 30; // deprecated: use ENTERPRISE
}

// Network — pass ALL to skip the dimension. Common members:
//   ALL, ETH_MAINNET, ETH_SEPOLIA, ETH_HOLESKY,
//   ETH_BSC_MAINNET, ETH_BSC_TESTNET, ETH_POLYGON_MAINNET, AMOY,
//   ARB_ONE, OPT_MAINNET, OPT_MAINNET_FULL,
//   SOL_MAINNET, NEAR_MAINNET, NEAR_TESTNET, BTC_MAINNET,
//   AVAL_MAINNET, AVALANCHE, EOSEVM,
//   SEI_EVM_MAINNET, SEI_EVM_DEVNET,
//   INJECTIVE_MAINNET, INJECTIVE_TESTNET,
//   STARKNET_MAINNET, STARKNET_TESTNET,
//   MANTRA_MAINNET, MANTRA_TESTNET,
//   STELLAR_MAINNET, STELLAR_TESTNET
// If the user names a network not listed, ask them for the exact enum name
// (chain support is added over time; the proto is the source of truth).
```

### Response messages

```proto
message Subscription {
  string      plan_name    = 1;
  string      plan_id      = 2;
  PlanTier    plan_tier    = 3;
  optional uint32 base_price = 4;     // cents; may be absent on community
  OverageType overage_type = 9;
  ServiceType service_type = 20;
  oneof service_config {
    SubstreamsServiceConfig substreams_config = 21;
    TokenAPIServiceConfig   token_api_config  = 22;
  }
}

message SubstreamsServiceConfig {
  BytesValue egress_quota                          = 1;
  optional BytesPrice egress_overage_price         = 2;
  uint32 block_count_quota                         = 3;
  optional UnitPrice block_count_overage_price     = 4;
  uint32 substreams_max_workers                    = 5;
  uint32 substreams_max_requests                   = 6;
  uint32 substreams_request_min_life_time_second   = 7;
}

message TokenAPIServiceConfig {
  uint32 items_returned                          = 1;
  uint32 batch_size                              = 2;
  TimeParameter min_time_parameter               = 3;  // enum, e.g. TIME_PARAMETER_ONE_MINUTE
  TimeInterval  time_interval                    = 4;
  uint32 rate_limit_per_minute                   = 5;
  bool   real_time_data                          = 6;
  TokenAPIGroup maximum_allowed_endpoint_group   = 7;  // TOKEN / DEX / NFT / HISTORICAL
  uint32 plan_credits_cents                      = 8;
  uint32 price_cents_token                       = 9;   // per million items
  uint32 price_cents_dex                         = 10;
  uint32 price_cents_nft                         = 11;
  uint32 price_cents_historical                  = 12;
  HistoricalPriceDataAvailability historical_data_availability = 13;
}

message BytesValue { uint32 value = 1; BytesUnit unit = 2; }
message BytesPrice { uint32 price_per_unit = 1; BytesUnit unit = 2; }  // cents per unit
message UnitPrice  { uint32 price_per_unit = 1; uint32 unit_count = 2; } // cents per group

enum BytesUnit {
  BYTES_UNIT_UNSPECIFIED = 0;
  BYTES_UNIT_GIBIBYTE    = 1;
  BYTES_UNIT_TEBIBYTE    = 2;
  BYTES_UNIT_MEBIBYTE    = 3;
}

message OrganizationBillingDetails {
  string         company_name = 1;
  BillingAddress address      = 2;
  string         tax_id       = 3;
  string         tax_id_type  = 4;
}

message BillingAddress {
  string line1 = 1; string line2 = 2;
  string city = 3;  string state = 4;
  string postal_code = 5; string country = 6;
}

message BillingPreview {
  BillingSummary summary = 1;
  BillingDetails details = 2;
}

message BillingSummary {
  // Substreams + Firehose (combined)
  uint32 base_price_cents          = 1;
  uint32 egress_overage_cents      = 2;
  uint32 blocks_overage_cents      = 3;
  bool   substreams_quota_exceeded = 4;
  bool   substreams_has_usage      = 11;

  // Token API
  uint32 token_api_base_price_cents = 5;
  uint32 token_api_credits_cents    = 6;
  uint32 token_api_cost_cents       = 7;
  uint32 token_api_overage_cents    = 8;
  bool   token_api_quota_exceeded   = 9;

  uint32 total_cents = 10;
}

message BillingDetails {
  SubstreamsBillingDetails substreams = 1;
  SubstreamsBillingDetails firehose   = 2;
  TokenAPIBillingDetail    token_api  = 3;
}

message SubstreamsBillingDetails {
  uint64 egress_bytes_used     = 1;
  uint64 egress_bytes_quota    = 2;
  uint64 blocks_used           = 3;
  uint64 blocks_quota          = 4;
  uint32 egress_overage_cents  = 5;
  uint32 blocks_overage_cents  = 6;
}

message TokenAPIBillingDetail {
  uint64 requests_total          = 1;
  uint32 overage_cents           = 2;
  uint64 requests_token          = 3;
  uint64 requests_dex            = 4;
  uint64 requests_nft            = 5;
  uint64 requests_historical     = 6;
  uint32 cost_cents_token        = 7;
  uint32 cost_cents_dex          = 8;
  uint32 cost_cents_nft          = 9;
  uint32 cost_cents_historical   = 10;
  uint32 plan_credits_cents      = 11;
  uint32 credits_applied_cents   = 12;
  uint32 total_cost_cents        = 13;
  uint32 base_price_cents        = 14;
}

message MultiServiceUserConsumption {
  ConsumptionBuckets substreams_processed_bytes  = 1;  // legacy
  ConsumptionBuckets firehose_egress_bytes       = 2;
  ConsumptionBuckets substreams_egress_bytes     = 3;
  ConsumptionBuckets substreams_processed_blocks = 4;
  ConsumptionBuckets token_api_requests_total    = 5;
  ConsumptionBuckets firehose_processed_blocks   = 6;
  ConsumptionBuckets token_api_requests_token    = 7;
  ConsumptionBuckets token_api_requests_dex      = 8;
  ConsumptionBuckets token_api_requests_nft      = 9;
  ConsumptionBuckets token_api_requests_historical = 10;
}

message ConsumptionBuckets {
  uint64 today         = 1;
  uint64 yesterday     = 2;
  uint64 current_month = 3;
  uint64 last_month    = 4;
  uint64 last_7_days   = 5;
  uint64 last_30_days  = 6;
}

message UserUsage {
  repeated Metric daily_metrics = 2;
  UserUsageMetadata metadata    = 3;
}

message Metric {
  string date                          = 1;  // YYYY-MM-DD
  uint64 bytes                         = 2;
  uint64 token_api_requests            = 3;
  uint64 firehose_egress_bytes         = 4;
  uint64 substreams_egress_bytes       = 5;
  uint64 substreams_processed_bytes    = 6;
  uint64 substreams_processed_blocks   = 7;
  uint64 firehose_processed_blocks     = 8;
  uint64 token_api_requests_token      = 9;
  uint64 token_api_requests_dex        = 10;
  uint64 token_api_requests_nft        = 11;
  uint64 token_api_requests_historical = 12;
}

message UserUsageMetadata {
  repeated string api_keys_with_usage = 1;  // IDs only
  repeated string networks_with_usage = 2;  // network names
}

message ActiveConnectionsResponse {
  int64 borrowed_workers   = 1;
  int64 max_workers        = 2;
  int64 available_workers  = 3;
  int64 active_requests    = 4;
  int64 max_requests       = 5;
  int64 available_requests = 6;
  map<string, int64> worker_count_per_trace_id = 7;
}
```

### Hosted-deployment response messages

Every hosted response carries a `bool success` + `string message` envelope; the payload sits in a nested field. On `success == false`, surface `message` to the user rather than the empty payload.

```proto
message ListDeploymentsResponse {
  bool   success = 1;
  string message = 2;
  repeated DeploymentInfo deployments = 3;
}

message DeploymentInfo {
  string         deployment_id   = 1;
  DeploymentState state          = 2;   // nested message; see below
  string         deployment_type = 3;   // e.g. "sink-sql-from-proto", "foundational-store"
  string         output_module   = 4;
  string         spkg_url        = 5;
  string         deployment_name = 6;
}

message GetDeploymentResponse {
  bool       success = 1;
  string     message = 2;
  Deployment deployment = 3;
}

// GetDeployment returns the stored deployment config (the full DeploymentRequest
// shape) plus the id of the API key bound to it. The config is verbose and
// network-specific; for status questions prefer GetDeploymentState. Surface
// deployment_id, name, and api_key_id; only dig into deployment_request when the
// user explicitly asks about config (spkg, output module, output target).
message Deployment {
  string deployment_id = 1;
  string name          = 2;
  sf.hosted.common.v1.DeploymentRequest deployment_request = 4;  // verbose; see note above
  string api_key_id    = 5;
  reserved 3;            // was api_key (raw secret) — removed; never returned
}

message DeploymentStateResponse {
  bool            success = 1;
  string          message = 2;
  DeploymentState deployment_state = 3;
}

message DeploymentState {
  ResourceState resource_state   = 1;   // CREATED | DELETING (control-plane record)
  Deployment    deployment_state = 2;   // DEPLOYING | DEPLOYED | DELETED | ERROR (K8s)
  int64 replica          = 10;
  int64 ready_replicas   = 11;
  int64 healthy_replicas = 12;
  bool  crashloopbackoff = 13;
  int64  max_restart_count     = 14;
  string pod_with_max_restarts = 15;
  repeated ExecutionState execution_states = 20;

  enum ResourceState {
    RESOURCE_STATE_CREATED  = 0;
    RESOURCE_STATE_DELETING = 1;
  }
  enum Deployment {            // K8s observed state (note: same name as the message above)
    DEPLOYMENT_STATE_DEPLOYING = 0;   // starting up, not all replicas ready, no hard failure
    DEPLOYMENT_STATE_DEPLOYED  = 1;
    DEPLOYMENT_STATE_DELETED   = 2;   // StatefulSet gone
    DEPLOYMENT_STATE_ERROR     = 99;
  }
}

message ExecutionState {
  string pod_name        = 1;
  string cursor          = 2;
  State  state           = 3;   // INITIATING | CATCHING_UP | LIVE | FAILING
  bool   initialized     = 10;
  bool   ready           = 11;
  bool   containers_ready = 12;
  bool   pod_scheduled   = 13;
  int64  restart_count   = 20;
  string container_state = 21;
  // started_at = 22 (timestamp)
  int64  current_block   = 25;
  int64  head_block      = 26;
  double head_block_time_drift = 27;  // seconds behind real-time
  repeated string events = 30;

  enum State {
    STATE_INITIATING  = 0;
    STATE_CATCHING_UP = 1;
    STATE_LIVE        = 4;
    STATE_FAILING     = 5;
  }
}

message GetDeploymentEventsResponse {
  bool   success = 1;
  string message = 2;
  repeated DeploymentEventProto events = 3;
}

message DeploymentEventProto {
  int64  id            = 1;
  string deployment_id = 2;
  string event_type    = 3;
  string details       = 4;   // JSON string
  string created_at    = 5;   // RFC3339
}

message LogsResponse {
  bool   success = 1;
  string message = 2;
  repeated PodLogs pod_logs = 3;
}
message PodLogs { string pod_name = 1; string logs = 2; }
```

### Hosted-deployment mutation responses

All share the `bool success` + `string message` envelope; most carry no further payload. Report `message` to the user verbatim on failure, and confirm the action succeeded on `success == true`.

```proto
message DeploymentResult {}                                    // Deploy — empty
message ReplicaResponse                { bool success = 1; string message = 2; }
message UndeployResponse               { bool success = 1; string message = 2; }
message ResetDeploymentResponse        { bool success = 1; string message = 2; }
message UpdateDeploymentConfigResponse { bool success = 1; string message = 2; }
message DeleteDeploymentConfigResponse { bool success = 1; string message = 2; }
message CleanupOrphanedDeploymentResponse { bool success = 1; string message = 2; }
message DeployDatabaseResponse {
  bool   success = 1;
  string message = 2;
  OutputConfig output_config = 3;   // resolved DB connection; password is scrubbed server-side
                                    // (blank). Still never echo connection secrets.
}
```

### Filter quick rules

- `date_filter`: pick from the enum (no custom date range field exists). For "last 6 months" or arbitrary windows, use `DATE_FILTER_CURRENT_YEAR` and slice `daily_metrics` locally by `date` prefix.
- `provider`: `PROVIDER_ALL` to skip; `PROVIDER_SF` or `PROVIDER_PINAX` to scope.
- `network`: `ALL` to skip; otherwise the specific enum value (see list above).
- `service`: `SERVICE_UNSET` to skip; otherwise `SERVICE_SUBSTREAMS` / `SERVICE_FIREHOSE` / `SERVICE_TOKEN_API`.
- `api_key_id`: omit or empty string to include all keys; set to a `key_...` to scope.

Never pass an empty string for an enum field — the request will fail with `invalid_argument`. Pass the explicit `*_UNSET` / `*_ALL` value.

### Unit reminders

- All `*_cents` fields are integer cents. Divide by 100 for dollars.
- `BytesValue.value` paired with `BytesUnit` — e.g. `value=100, unit=BYTES_UNIT_GIBIBYTE` means 100 GiB.
- Daily metric `bytes` and `*_egress_bytes` are raw `uint64`. Format with a humanized unit (GiB / TiB) for display.

## Rendering & Humanization Rules

**Never show raw enum strings, raw cents, or raw byte counts to the user.** Always translate to the friendly form below before rendering. Raw values appear only when the user explicitly asks for "raw" or "json".

### Enum → human label

| Enum value | Display as |
|---|---|
| `PLAN_TIER_COMMUNITY` | Community |
| `PLAN_TIER_SCALING` | Scaling |
| `PLAN_TIER_PRO` | Pro |
| `PLAN_TIER_ENTERPRISE` | Enterprise |
| `PLAN_TIER_METERED` | Metered (legacy) |
| `PLAN_TIER_CUSTOM` | Custom (legacy) |
| `OVERAGE_TYPE_STRICT` | Hard cap (no overages) |
| `OVERAGE_TYPE_METERED` | Overages allowed (metered) |
| `OVERAGE_TYPE_UNSPECIFIED` | Unknown |
| `SERVICE_TYPE_SUBSTREAMS` | Substreams |
| `SERVICE_TYPE_TOKEN_API` | Token API |
| `SERVICE_SUBSTREAMS` | Substreams |
| `SERVICE_FIREHOSE` | Firehose |
| `SERVICE_TOKEN_API` | Token API |
| `BYTES_UNIT_GIBIBYTE` | GiB |
| `BYTES_UNIT_TEBIBYTE` | TiB |
| `BYTES_UNIT_MEBIBYTE` | MiB |
| `TOKEN_API_GROUP_TOKEN` | Token |
| `TOKEN_API_GROUP_DEX` | DEX |
| `TOKEN_API_GROUP_NFT` | NFT |
| `TOKEN_API_GROUP_HISTORICAL` | Historical |
| `PROVIDER_SF` | StreamingFast |
| `PROVIDER_PINAX` | Pinax |
| `PROVIDER_ALL` | All providers |
| `DATE_FILTER_TODAY` | today |
| `DATE_FILTER_YESTERDAY` | yesterday |
| `DATE_FILTER_LAST_7_DAYS` | last 7 days |
| `DATE_FILTER_LAST_30_DAYS` | last 30 days |
| `DATE_FILTER_LAST_MONTH` | last month |
| `DATE_FILTER_CURRENT_MONTH` | this month |
| `DATE_FILTER_CURRENT_YEAR` | this year |
| `DATE_FILTER_LAST_YEAR` | last year |
| `TIME_PARAMETER_ONE_MINUTE` | 1 min |
| `TIME_PARAMETER_FIVE_MINUTES` | 5 min |
| `TIME_PARAMETER_TEN_MINUTES` | 10 min |
| `TIME_PARAMETER_THIRTY_MINUTES` | 30 min |
| `TIME_PARAMETER_ONE_HOUR` | 1 hour |
| `TIME_PARAMETER_FOUR_HOURS` | 4 hours |
| `TIME_PARAMETER_ONE_DAY` | 1 day |
| `TIME_INTERVAL_168_BARS` | 168 bars (7 days @ 1h) |
| `TIME_INTERVAL_2160_BARS` | 2160 bars (90 days @ 1h) |
| `TIME_INTERVAL_FULL` | full history |
| `HISTORICAL_PRICE_DATA_AVAILABILITY_6_MONTHS` | 6 months |
| `HISTORICAL_PRICE_DATA_AVAILABILITY_2_YEARS` | 2 years |
| `HISTORICAL_PRICE_DATA_AVAILABILITY_FULL` | full history |
| `DEPLOYMENT_STATE_DEPLOYING` | Deploying |
| `DEPLOYMENT_STATE_DEPLOYED` | Running ✅ |
| `DEPLOYMENT_STATE_DELETED` | Deleted |
| `DEPLOYMENT_STATE_ERROR` | Error ⚠ |
| `RESOURCE_STATE_CREATED` | Created |
| `RESOURCE_STATE_DELETING` | Deleting |
| `STATE_INITIATING` | Initiating |
| `STATE_CATCHING_UP` | Catching up |
| `STATE_LIVE` | Live ✅ |
| `STATE_FAILING` | Failing ⚠ |

For an enum value not in this table (e.g. a `Network` member), title-case the suffix after the prefix. Example: `ETH_MAINNET` → "Ethereum Mainnet", `OPT_MAINNET` → "Optimism Mainnet", `SOL_MAINNET` → "Solana Mainnet". When unsure of the canonical product name, ask the user or fall back to the raw token in backticks so it's clearly an identifier (e.g. `` `STARKNET_MAINNET` ``).

### Numbers → friendly form

- **Money**: `*_cents` → `$X.XX` (divide by 100, two decimals, omit trailing zeros on whole dollars). `12300` → `$123`. `12345` → `$123.45`.
- **Bytes**: raw `uint64` → IEC binary unit with one decimal. `1_073_741_824` → `1.0 GiB`. Pick the smallest unit ≥ 1.0 (MiB / GiB / TiB).
- **Block counts / request counts**: raw `uint64` → thousands separators. `1234567` → `1,234,567`. Above 1M, prefer `1.23M` notation when space-constrained.
- **`base_price` (uint32 cents)**: divide by 100, append `/mo` if showing as a plan price. `9900` → `$99/mo`.
- **IDs**: render in backticks. Optionally truncate long opaque IDs to first 8 + last 4 with `...` in the middle when listing many (e.g. `0cyje0b9...98e6`); keep full ID in the source for any copy-out.

### Plan summary template

When the user asks "what's our plan", render this shape (fill in available fields, skip the rest):

> **{company_name}** — **{plan_name}** ({plan_tier} tier, {service_type}). Base **{base_price}**, **{overage_type}**.
>
> Quotas: **{egress_quota}** egress, **{block_count_quota}** blocks. Workers: **{substreams_max_workers}** max, **{substreams_max_requests}** parallel requests.

Token-API plans substitute the quota line:

> Quotas: **{plan_credits_cents}** credits, **{rate_limit_per_minute}** req/min, access up to **{maximum_allowed_endpoint_group}**, **{historical_data_availability}** historical depth.

### Bill summary template

> Projected total: **{total_cents}**. Substreams/Firehose: **{base_price_cents}** base + **{egress_overage_cents}** egress overage + **{blocks_overage_cents}** blocks overage. Token API: **{token_api_cost_cents}** ({token_api_credits_cents} credits applied, **{token_api_overage_cents}** overage).

Surface `substreams_quota_exceeded` / `token_api_quota_exceeded` as a separate inline warning (e.g. "⚠ Substreams quota exceeded this period.").

### Usage summary template

For `MultiServiceUsageSummaryByOrganization`, render one row per non-zero service:

> | Service | Today | This month | Last 30 days |
> |---|---|---|---|
> | Substreams egress | 4.2 GiB | 120 GiB | 98 GiB |
> | Substreams blocks | 1.2M | 38M | 31M |
> | Firehose egress | 0 | 0 | 0 |
> | Token API requests | 12,300 | 1,234,567 | 980,000 |

Skip rows where all buckets are zero — don't show empty noise.

### Deployment summary template

For `ListDeployments`, render one row per deployment, sorted with errors/failing first:

> | Deployment | Status | Type | Output module |
> |---|---|---|---|
> | `my-sink` (`dep_abc`) | Running ✅ | sink-sql-from-proto | `db_out` |
> | `nft-store` (`dep_def`) | Error ⚠ | foundational-store | `graph_out` |

For `GetDeploymentState`, lead with the headline status, then per-pod execution detail when relevant:

> **`my-sink`** — Running ✅ (3/3 replicas healthy). Pod `my-sink-0`: **Live**, head block **21,402,118**, **2.4s** behind real-time. No restarts.

Flag trouble inline: `crashloopbackoff == true`, `deployment_state == DEPLOYMENT_STATE_ERROR`, any `ExecutionState.state == STATE_FAILING`, or a high `max_restart_count` → surface as a ⚠ warning with `pod_with_max_restarts` / `container_state`.

Humanize `head_block_time_drift` as "X behind real-time" (seconds → "Ns", "Nm", or "live" when ≈ 0). Render block numbers with thousands separators.

## Mutations — Confirmation Protocol

Read routes never need confirmation. Every mutating `HostedService` route does. Before calling one:

1. **Restate the action** in plain language: which deployment (name + id), which org, and exactly what changes.
2. **Flag the blast radius.** Mark destructive/irreversible ops explicitly — `ResetDeployment` (drops or truncates the **user's** DB schema/tables), `Undeploy`, `DeleteDeploymentConfig`, `CleanupOrphanedDeployment`. Note when an action creates **billable hosted sink-runner** infrastructure (`Deploy`) or pauses processing (`SetReplica count:0`). **`DeployDatabase` does not provision a database** — it only attaches/validates the user's existing Postgres connection (billable impact is the sink runner via `Deploy`, not a StreamingFast-hosted DB).
3. **Get an explicit yes** for that specific action. A general "manage my sinks" request is not consent to a destructive call. Don't batch multiple mutations behind one confirmation.
4. **Secrets**: never accept a DB password inline or put one in a request you compose — route it through the stored-secret flow ("Provide the database password" above). Never echo DB passwords or raw API-key secrets back; redact when summarizing the body.
5. **After the call**, report `success`/`message`, then offer a `GetDeploymentState` follow-up so the user sees the result.

If the server returns `permission_denied`, the logged-in user lacks the OWNER/ADMIN role required to mutate — surface that; do not retry.

## Conversational Playbook

Default to a short prose answer plus a small markdown table when multiple numbers matter. No raw JSON unless the user explicitly asks ("raw"/"json").

### "What's our current plan / subscription?"

Call `GetOrganizationSubscription` + `GetBillingDetails`. Translate enums to friendly names: `PLAN_TIER_PRO` → "Pro", `OVERAGE_TYPE_METERED` → "overages allowed (metered)", `OVERAGE_TYPE_STRICT` → "hard cap". Convert `base_price` cents → dollars. Surface quotas from `substreams_config` / `token_api_config`.

> Acme Inc. is on the **Pro** tier for Substreams. Base $99/mo, overages metered. Quota: 100 GiB egress + 50M blocks. 16 max workers, 32 max parallel requests.

### "What's our usage this month?"

Call `MultiServiceUsageSummaryByOrganization`. For each non-zero service, report the `current_month` bucket. For a trend, follow up with `UsageByOrganization` (`DATE_FILTER_CURRENT_MONTH`) and render daily totals.

### "What's our usage for API key `key_xxx` over the last 6 months?"

`DateFilter` has no 6-month enum. Use `DATE_FILTER_CURRENT_YEAR`, scope to `api_key_id: "key_xxx"`, then locally sum the last ~180 days of `daily_metrics`. Tell the user "Returned the trailing 6 months from your current-year usage." If they need precision, run `LAST_MONTH` + `CURRENT_MONTH` for the 2-month tail and stitch.

### "What will our next bill be?"

Call `GetUsageBilling` with `{"usage":{}}`. Report `summary.total_cents / 100` as the headline. If `substreams_quota_exceeded` or `token_api_quota_exceeded` is true, surface that. Break out per-service from `details.substreams`, `details.firehose`, `details.token_api`.

### "Should we upgrade / downgrade?"

Rubric, in order:

1. `GetOrganizationSubscription` — current `plan_tier`, `overage_type`, `base_price`, `service_config` quotas.
2. `GetUsageBilling` (`{"usage":{}}`) — projected `total_cents`.
3. `MultiServiceUsageSummaryByOrganization` — raw consumption (`current_month` bucket).
4. Compare (both sides are **integer cents** — do **not** multiply `base_price` by 100):
   - `total_cents` ≤ `base_price` **and** consumption < 50% of quota for two periods → suggest downgrade.
   - `total_cents` > `base_price` **and** `overage_type == OVERAGE_TYPE_METERED` → paying overages; if a higher tier's base price would undercut projected total, suggest upgrade.
   - `total_cents` > `base_price` **and** `overage_type == OVERAGE_TYPE_STRICT` (or `substreams_quota_exceeded == true`) → hitting cap; strongly suggest upgrade.
   - Else → hold.
5. Phrase as advice. Do **not** change the plan — billing/subscription mutations are out of scope (only the Portal UI does that).

### "Who's connected right now?"

Call `ActiveConnections`. Lead with the load fraction (`borrowed_workers / max_workers`, `active_requests / max_requests`). If `worker_count_per_trace_id` has entries, render a short table sorted by worker count desc.

> 7/16 workers in use, 3/32 active requests. Top traces: `abc123` (4 workers), `def456` (3 workers).

### "List my deployments / what's deployed?"

Call `HostedService/ListDeployments` with `{"organization_id": "<ORG_ID>"}`. Render the deployment table (status, type, output module). If `success == false`, surface `message`. If `deployments` is empty, say there are no hosted deployments for the org.

### "Is my sink running? / what's its status?"

If you don't have a `deployment_id`, call `ListDeployments` first and match by name. Then call `HostedService/GetDeploymentState` with `{"deployment_id": "...", "organization_id": "..."}`. Lead with the headline status (`deployment_state` + healthy/ready replica fraction), then per-pod execution detail (state, `current_block` / `head_block`, `head_block_time_drift`). Flag `crashloopbackoff`, `STATE_FAILING`, `DEPLOYMENT_STATE_ERROR`, or high restart counts as warnings.

### "How far behind is my sink? / what's the head block?"

`GetDeploymentState` → read `execution_states[].head_block` and `head_block_time_drift`. Report the head block and lag ("2.4s behind real-time" / "live"). With multiple pods, report each or the worst-lagging one.

### "What happened to deployment `xxx`? / show recent events."

Call `HostedService/GetDeploymentEvents` with `{"deployment_id": "...", "organization_id": "...", "limit": 20}`. Render newest-first as `created_at` — `event_type` — summarized `details` (the `details` field is a JSON string; parse and summarize key fields, don't dump it raw). Useful for explaining an ERROR or crashloop seen in `GetDeploymentState`.

### "Show me the logs for deployment `xxx`"

Call `HostedService/Logs` with `{"deployment_id": "...", "organization_id": "...", "tail_lines": 200}`. Set `previous: true` to pull logs from a crashed container's prior instance (useful when `GetDeploymentState` shows crashloop). Render per-pod, trimmed; highlight error/panic lines rather than dumping everything.

### "Deploy this spkg as a SQL sink" (mutation — confirm first)

Follow **`substreams-hosted-sink`** for the full sink workflow (that skill owns the deploy path). Two hard gates before Deploy:

1. **`OUTPUT_TEST_STATUS` must be set** — before Portal login / secrets / `Deploy` / any “ready to deploy hosted …?” prompt, load `substreams-hosted-sink` and run its **⛔ STOP — Output quality gate**: offer as its **own turn**, then **give a ready-to-paste `substreams run` command** (stdout only). **Do not** run a local sink or sync to ClickHouse/Postgres for this check. **Do not infer** verification from “we built the package.”
2. **spkg must be internet-reachable** — publish on **[substreams.dev](https://substreams.dev)**, then set:

```text
spkg.url = https://api.substreams.dev/v1/packages/<package-name>/<version>
```

(e.g. `https://api.substreams.dev/v1/packages/raydium-clmm/v0.1.4`). GitHub Release / public CDN URLs are also fine. **Prefer `url` over `substreams_dev_id`** for hosted runners. Do **not** Deploy a local path or private URL.

Gather inputs **one field at a time** (choices + custom where applicable) — never a single multi-field form:

1. **Output quality offer first** — mandatory `substreams-hosted-sink` STOP gate (`OUTPUT_TEST_STATUS`): hand the user a **`substreams run`** command only — **not** a sink/sync. **Forbidden:** “Ready to proceed to the hosted ClickHouse/Postgres deployment?” while unset.
2. Deployment **name**
3. **spkg source** — public HTTPS URL (API download preferred) · Other/custom (must still be public)
4. **network** (e.g. `solana`, `ethereum-mainnet` — Substreams network id)
5. **output_module** / **module_output_type**
6. **Database connection fields one by one** (never password in chat):
   - host/server → port → user → database → schema (Postgres) → sslmode / secure (as applicable)
7. **replica** (default 1)

Deploy confirmation must restate how quality was handled (`tested_ok` / `user_verified` / `skipped`).

**Password:** secure stored-secret flow only — see below. Sequence: (1) mint `deployment_id` via `CreateDeployment`; (2) user stages password on the secure page — **wait for user confirmation**, then **one** `HasDeploymentSecret` check (no background poll); (3) confirm plan; (4) `Deploy` with `use_stored_secret: true` and empty `password`. Do **not** supply an API key. **`Deploy` may return HTTP 200 + empty `{}`** — treat as success and follow up with `GetDeploymentState`. If the first `Deploy` fails with a connection timeout/refused against a serverless output DB (cold start), retry 2–3 times waiting ~30–60s — tell the user the DB may be waking up; only treat auth/host-not-found as hard failures immediately.

### "Provide the database password" / attach the output DB (mutation — confirm first)

The database password must never pass through you. Use this flow so the user enters it once, directly in the browser:

1. **Mint / hold a `deployment_id`** (`CreateDeployment` — never invent a slug). Secret is scoped to that id.
2. **Send the user to the secure page on the Portal UI host** — **not** `admin.streamingfast.io` (API only; `/sinks/...` **404**s there).

   | Engine | Full URL |
   |---|---|
   | PostgreSQL | `https://thegraph.market/sinks/<deployment_id>/secret?output=postgres` |
   | ClickHouse | `https://thegraph.market/sinks/<deployment_id>/secret?output=clickhouse` |

   Also valid: `https://app.streamingfast.io/sinks/<deployment_id>/secret?output=postgres|clickhouse`.

   **Do not offer to take the password yourself.** Emit the callout and **end the turn**.
3. **Wait for user confirmation** ("I've saved it") — do **not** background-poll `HasDeploymentSecret` (no sleep loops, no background tasks). Same rule as Portal device login / The Graph Market hand-offs: **user confirms first**.
4. **Only after they confirm**, call `HasDeploymentSecret` **once**. If `exists: false`, tell them it isn't stored yet and ask them to save again / tell you when ready — still no poll loop.
5. **Confirm, then call `DeployDatabase` / `Deploy`** with `use_stored_secret: true` and empty `password`.
6. Report `success`/`message`; password on `output_config` is blanked — expected.

If a user pastes a password into the chat, **decline to use it**: explain it would land in the transcript, and point them at the secure page instead.

### Collecting database connection info (one by one)

When the user brings their own Postgres/ClickHouse, ask **one question per turn** (with choices + **Other / custom** where useful):

| Order | Field | Notes |
|---|---|---|
| 1 | Host / server | Must be reachable from StreamingFast (not `localhost` / Docker-only names) |
| 2 | Port | Defaults: 5432 Postgres; ClickHouse native **9000**, Cloud TLS often **9440** |
| 3 | User | |
| 4 | Database name | |
| 5 | Schema | Postgres only (often `public`) |
| 6 | SSL / secure | Postgres `sslmode`; ClickHouse `secure` (true for Cloud) |
| — | Password | **Never in chat** — Portal secret page only |

Do not ask for all of these in one message.

### "Scale my sink to N replicas / pause it" (mutation — confirm first)

`SetReplica` with `{"deployment_id":"...","count":N,"organization_id":"..."}`. `count: 0` pauses (no workers, stops processing). Confirm the target count, then report the new state.

### "Undeploy / reset / delete deployment `xxx`" (destructive — confirm explicitly)

- `Undeploy` tears down the running deployment.
- `ResetDeployment` with `drop_schema:true` **drops the database schema**; `false` truncates tables. Both wipe sink data and restart from the configured start block.
- `DeleteDeploymentConfig` removes the stored config; `CleanupOrphanedDeployment` clears a dangling record.

For all of these: state plainly what is destroyed and that it is irreversible, name the deployment, and require an explicit "yes" (see Confirmation Protocol). Never chain a destructive call automatically off an ambiguous request.

### "Reconfigure / update deployment `xxx`" (mutation — confirm first)

`UpdateDeploymentConfig`. Only set the fields being changed; leave `api_key_id` empty to keep the bound key. `restart_from_scratch:true` resets the cursor (foundational-store only) — call that out as data-affecting. Confirm the diff, then apply.

## Default Render — Full Dashboard

When the user asks broadly ("give me the state of our account", "summary"), fan out five calls in parallel and assemble four cards:

1. **Header** — company name + plan name + tier + service type.
2. **Bill** — `summary.total_cents` (cents → dollars) + per-service line items from `details`.
3. **Usage** — `MultiServiceUsageSummaryByOrganization`, one row per non-zero service showing `current_month` (and `today` if interesting).
4. **Live** — `ActiveConnections` workers/requests fractions + per-`trace_id` rows.

Skip the daily chart by default; offer it as a follow-up.

## Error Handling

ConnectRPC returns HTTP status + JSON `{ "code": "...", "message": "...", "details": [...] }`. The `details` array carries an `sf.portalapi.v1.ErrorDetail` whose `debug.fields` names the offending field on `invalid_argument` — use it to locate a bad field instead of guessing. Report `message` to the user, not the raw envelope.

A **plain-text `404 page not found`** (not JSON) means the method name or service path is wrong — check the `{Service}/{Method}` spelling rather than treating it as a Portal error.

| Code | Meaning | Action |
|---|---|---|
| `unauthenticated` | Access token expired/invalid, or route not in allowlist (`api/auth/agent_allowed_routes.go`) | The access token has most likely expired — refresh it (or re-run the device-code login) per `portal-api-jwt`, then retry once. Do not silently retry with the same expired token. |
| `permission_denied` | Logged-in user not in the target org, or lacks the role for the action (deployment mutations need OWNER/ADMIN via `CanUserManageDeployments`) | Tell the user the token isn't scoped to this `organization_id`, or they lack permission for that action. Do not retry. |
| `not_found` | Bad `organization_id` | Ask user to re-check the org ID. |
| `invalid_argument` | Malformed body — usually a misspelled enum, empty-string for enum, or missing required field | Fix the body using the proto inline above. |
| `unavailable` / `deadline_exceeded` | Transient | One retry with backoff is fine. |

Never include the access token or refresh token in error output shown to the user.

## Maintaining This Skill

The proto fragments above are inlined snapshots of the upstream definitions in `sf-saas-priv` (`proto/sf/portalapi/v1/portalapi.proto`, `hosted_service.proto`, `proto/sf/common/v1/types.proto`, and the `sf.hosted.common.v1` buf dependency). When the upstream proto changes in a way that affects one of the documented routes (new field, renamed field, new enum value, new method signature), update SKILL.md to match and bump `metadata.version`. The agent-callable set is gated server-side by `api/auth/agent_allowed_routes.go` — if a route isn't in that map it returns `unauthenticated` no matter what this doc says, so keep the two in sync. Note `HasDeploymentSecret` IS agent-callable but `StoreDeploymentSecret` is deliberately NOT (the password is entered by the user in the web UI, so an agent never writes it) — don't add it to the allowlist or call it from here. The `HostedService` payload types live in the external `buf.build/streamingfast/hosted-service` module, so they can change independently of `sf-saas-priv`.

**Verifying this skill against the live server.** The production server exposes gRPC
reflection, which is the authoritative check and needs no repo access and no token:

```bash
grpcurl admin.streamingfast.io:443 list
grpcurl admin.streamingfast.io:443 describe sf.portalapi.v1.HostedService
grpcurl -protoset-out portal.protoset admin.streamingfast.io:443 describe sf.portalapi.v1.PortalApi
grpcurl -protoset portal.protoset describe sf.common.v1.Subscription   # then offline
```

Reflection resolves the imported `sf.hosted.common.v1.*` types too, so it covers the
whole surface documented here.

If the assistant ever sees `invalid_argument` for what should be a valid body, or a response missing a documented field, the most likely cause is that this skill is out of date. Tell the user. Note the inverse failure is quieter and more likely: because unknown fields are **silently discarded**, a field this skill documents but the server has since removed produces a **successful call that ignores the setting**, with no error at all. When a config doesn't take effect, re-check the field against reflection before assuming a server bug.

## Things the Assistant Should NOT Do

- Do not write or store the access token or refresh token to disk without explicit user consent.
- Do not call routes outside the ones listed above — the server allowlist will reject them with `unauthenticated`. The skill covers the six `PortalApi` billing/usage reads plus the full `HostedService`; unrelated `PortalApi` routes (key/org/payment management) are not callable.
- Do not mutate plan, billing, payment methods, or API keys — those `PortalApi` routes are out of scope. Deployment mutations ARE in scope, but only after explicit per-action confirmation (see Confirmation Protocol).
- Do not call a destructive deployment route (Undeploy, ResetDeployment, DeleteDeploymentConfig, CleanupOrphanedDeployment) without naming the target and getting an explicit yes.
- Do not accept a database password in the chat or place one in a request you build. Route it through the stored-secret flow (user enters it in the web UI; deploy with `use_stored_secret: true`). Do not call `StoreDeploymentSecret` — it is not on the agent surface.
- Do not **background-poll** Portal for device login or `HasDeploymentSecret` — wait for user confirmation, then one check.
- Do not ask “ready to deploy / proceed to hosted ClickHouse (or Postgres)?” or call `Deploy` until `OUTPUT_TEST_STATUS` is set via the `substreams-hosted-sink` **⛔ STOP** gate (offer → **`substreams run` command for the user** / user_verified / skip). Do **not** use a local sink or DB sync as that quality check. Building or publishing is not enough.
- Do not **Deploy** a hosted sink unless the `.spkg` is fetchable via **public HTTPS** (prefer `https://api.substreams.dev/v1/packages/<name>/<version>` after publish).
- Do not send secret-page links on **`admin.streamingfast.io`** — use **thegraph.market** or **app.streamingfast.io**.
- Do not ask for all database connection fields in one message — **one field per turn**.
- Do not paste raw JSON at the user unless they explicitly ask. Summarize.
- Do not generate language clients or codegen unless the user explicitly asks for an integration — this skill is for answering questions, not bootstrapping projects.
