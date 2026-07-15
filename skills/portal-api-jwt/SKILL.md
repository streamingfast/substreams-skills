---
name: portal-api-auth
description: Obtain and refresh a short-lived StreamingFast Portal access token via the device-code login flow (OAuth 2.0 Device Authorization Grant), so the agent can then call the Portal API on the user's behalf. Use when the agent has no API key and needs the user to log in interactively — e.g. "connect to my portal account", "log me in", "I don't have an API key", or when a Portal API call returns 401/unauthenticated and a token must be acquired or renewed. Produces an access token (Bearer) scoped to one organization and the user's role, plus a refresh token to renew it without re-prompting.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.5.0
  author: StreamingFast
  documentation: https://docs.substreams.dev
---

# StreamingFast Portal API — Device-Code Login & Token Refresh

This skill teaches the assistant to get a Portal **access token** by sending the
user through an interactive browser login, then keep that token fresh. It is the
authentication front-end for the [`portal-api`](../portal-api/SKILL.md) skill:
once you hold an access token, you call Portal API methods exactly as that skill
describes, with `Authorization: Bearer <access_token>`.

This is the standard way to authenticate to the Portal API. Use it whenever you
need to call Portal routes and don't already hold a valid access token — the
first time, or when an existing token has expired and must be renewed.

## The Flow at a Glance (RFC 8628 Device Authorization Grant)

1. **Start** — `POST DeviceAuthorize`. Get a `user_code`, a `verification_uri`, and a secret `device_code`.
2. **Hand off to the user** — show them the URL + `user_code` and **stop**. Ask them to approve in their browser and **tell you when done**.
3. **Wait for user confirmation** — do **not** background-poll `DeviceToken` while they are away. No sleep loops, no background tasks, no “I'll poll until you approve.”
4. **After the user confirms** — call `POST DeviceToken` once with the `device_code`. If still `PENDING`, say so and wait for them again (or one short retry only after they re-confirm) — never a continuous poll loop.
5. **Receive tokens** — on approval you get an `access_token` (short-lived) and a `refresh_token` (longer-lived).
6. **Call the API** — use `Authorization: Bearer <access_token>` on Portal API calls.
7. **Refresh** — when the access token nears/passes expiry, `POST RefreshToken` to get a new pair. Never re-prompt the user until the refresh token itself expires or is revoked.

The agent **never** calls the approval endpoint — that happens in the user's
browser. The agent only calls `DeviceAuthorize`, `DeviceToken`, and `RefreshToken`,
all of which are **public** (no auth header needed).

## Setup

| Var | Default | Notes |
|---|---|---|
| `BASE_URL` | `https://admin.streamingfast.io` | User says "local"/"localhost" → `http://localhost:9000`. Other host → use as-is. |

All three endpoints are `POST` of a JSON body to `{BASE_URL}/sf.portalapi.v1.PortalApi/<Method>`.
Prefer **snake_case** in request bodies. **Responses may use camelCase** — parse both.
Enum values are full strings (e.g. `"DEVICE_TOKEN_STATUS_APPROVED"`). Call with the
Bash tool + `curl`, pipe through `jq`.

## Step 1 — Start the login

```bash
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.PortalApi/DeviceAuthorize" \
  -H "Content-Type: application/json" \
  -d '{"client_name": "Claude agent"}'
```

Response (wire JSON may be **snake_case or camelCase** — parse both):

```json
{
  "device_code": "device_9f8c…",
  "deviceCode": "device_9f8c…",
  "user_code": "BCDF-GH2J",
  "userCode": "BCDF-GH2J",
  "verification_uri": "https://thegraph.market/device",
  "verificationUri": "https://thegraph.market/device",
  "verification_uri_complete": "https://thegraph.market/device?user_code=BCDF-GH2J",
  "verificationUriComplete": "https://thegraph.market/device?user_code=BCDF-GH2J",
  "expires_in": 600,
  "expiresIn": "600",
  "interval": 5
}
```

Keep `device_code` / `deviceCode`, `interval`, and `expires_in` / `expiresIn` in memory.
**Do not print the `device_code`** — it is the agent's secret half of the handshake.
Use `verification_uri_complete` (or camelCase) as returned; production often points at
**thegraph.market**, not only admin.streamingfast.io.

## Step 2 — Send the user to approve

This message is easy to lose in a wall of text — make it **stand out**. Output the
callout **exactly as the template below** — do not reword it, flatten it into a
sentence, or drop the heading or blockquote. Substitute only the `{...}`
placeholders; keep every line, both `###` headings, the `>` blockquote bar (the
terminal draws a colored rule), and the emojis verbatim. The URL must stay on its
own line as **bare text in the blockquote** (never inside backticks or a fenced
code block) so the terminal renders it as a clickable blue link. Show the link
**once** — do not repeat it as a manual-entry fallback.

Make this a **focused, blocking ask**: the login is a clear gate, so the callout
should be the only thing in the turn. **Do not** pair it with narration about other
work happening "in parallel" or "meanwhile" (e.g. "Let me start the login and, in
parallel, inspect the spkg") — splitting the user's attention between an action they
must take and background agent work is confusing. **Do not** start a background
`DeviceToken` poll, sleep loop, or subagent while waiting. Hold all other work until
**after** the user confirms they approved and you have tokens.

Emit this and nothing else for the prompt:

> ### 🔓 Log in to approve access
>
> ⏳ **Expires in 10 minutes** — I'll wait for you; I won't poll in the background.
>
> **1.** Open this link:
>
> {verification_uri_complete}
>
> **2.** Sign in
> **3.** Choose the organization to grant
> **4.** Click Approve ✅
>
> ### 👉 Tell me once you've approved — only then will I fetch the token.

Prefer `verification_uri_complete` (one click, code pre-filled). The user selects
which organization the agent may act on and the token inherits **their role** in
that org.

## Step 3 — After the user confirms (no background poll)

**Hard rule:** never run a continuous or background poll for login approval (no
`while true` + sleep, no background shell, no “I'll keep checking”). The turn that
shows the login link must **end**. Resume only when the user says they approved
(or equivalent confirmation).

Only **after** that confirmation, call `DeviceToken` once:

```bash
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.PortalApi/DeviceToken" \
  -H "Content-Type: application/json" \
  -d '{"device_code": "device_9f8c…"}'
```

| `status` | Meaning | Action |
|---|---|---|
| `DEVICE_TOKEN_STATUS_PENDING` | Not approved yet | Tell the user it isn't registered yet; ask them to finish Approve and **tell you again** — do **not** enter a sleep/poll loop |
| `DEVICE_TOKEN_STATUS_SLOW_DOWN` | Called too fast | Wait for the next user message, then retry once |
| `DEVICE_TOKEN_STATUS_DENIED` | User denied | Stop; tell the user access was denied |
| `DEVICE_TOKEN_STATUS_EXPIRED` | Handshake timed out | Stop; restart at Step 1 |
| `DEVICE_TOKEN_STATUS_APPROVED` | Done | Capture the tokens (below) |

On `APPROVED` (fields may be camelCase: `accessToken`, `refreshToken`, `organizationId`, `expiresIn`):

```json
{
  "status": "DEVICE_TOKEN_STATUS_APPROVED",
  "access_token": "eyJ…",
  "refresh_token": "agentrt_…",
  "expires_in": 900,
  "organization_id": "0cyje0…"
}
```

**Capture `organization_id` / `organizationId`** — it is the org the user approved, and the **only**
place you learn it. Send this exact value as `organization_id` on every Portal API
call (Step 4). Don't guess it.

The `device_code` is **single-use** — once approved and the tokens are returned,
calling again fails. Stop.

Do **not** use the waiting period for unannounced background prep that looks like
parallel work; finish login first, then gather connection details / spkg URLs.

## Step 4 — Call the Portal API with the access token

Use the `portal-api` skill's methods, swapping the auth header:

```bash
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.HostedService/ListDeployments" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"organization_id": "<ORG_ID>"}'
```

Scope rules baked into the token:

- The token is pinned to **one organization**. Every request's `organization_id`
  **must match** the org the user approved, or the call is rejected
  (`permission_denied` / "unauthorized"). You don't choose the org per call — it
  was fixed at approval. Use the `organization_id` from the approved token response
  (Step 3); to act on a different org, have the user re-run the login.
- The token carries the user's **role** in that org; you can only do what that
  user could do (reads for any member; deploy/scale/undeploy need OWNER/ADMIN).
- Agent tokens may only call the org-scoped, allowlisted routes — the billing/
  usage routes and the full HostedService.

## Step 5 — Refresh before/after expiry

When the access token is near expiry (or a call returns unauthenticated), renew
**without** re-prompting the user:

```bash
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.PortalApi/RefreshToken" \
  -H "Content-Type: application/json" \
  -d '{"refresh_token": "agentrt_…"}'
```

Response — a **new** access token **and a new refresh token**:

```json
{
  "access_token": "eyJ…",
  "refresh_token": "agentrt_…NEW",
  "expires_in": 900
}
```

**Critical — refresh tokens are one-time-use (rotated):**

- Each refresh **invalidates the refresh token you just sent** and returns a new
  one. **Always replace your stored refresh token with the new value.**
- **Never reuse an old refresh token.** Presenting an already-rotated token is
  treated as theft and **revokes the entire token family** — you'll have to send
  the user through the login again.
- The refresh token has an **absolute lifetime** (~8h) that does **not** extend on
  rotation. When it finally expires, refresh fails — restart at Step 1.
- A refresh re-checks the user's live role/membership. If they were removed from
  the org or lost access, refresh fails — restart the login.

If `RefreshToken` returns an error (revoked, expired, or unknown), discard both
tokens and begin a fresh device-code login (Step 1).

## Token Lifetimes (defaults; deployment may tune)

| Token | Lifetime | Notes |
|---|---|---|
| Device-code handshake | ~10 min | `expires_in` from `DeviceAuthorize`; user must approve before it elapses |
| Access token | ~15 min | Short-lived, stateless Bearer JWT |
| Refresh token | ~8 h absolute | Rotated on every use; absolute deadline survives rotation |

## Quick Reference — Endpoints (all public, no auth)

```
POST {BASE_URL}/sf.portalapi.v1.PortalApi/DeviceAuthorize   {"client_name"?: string}
POST {BASE_URL}/sf.portalapi.v1.PortalApi/DeviceToken       {"device_code": string}
POST {BASE_URL}/sf.portalapi.v1.PortalApi/RefreshToken      {"refresh_token": string}
```

Then call any allowlisted Portal API / HostedService method with
`Authorization: Bearer <access_token>` and a matching `organization_id`.

## Handling Notes

- Treat `access_token` and `refresh_token` as secrets; never echo them to the user.
- Show the user only the `verification_uri`(`_complete`) and `user_code`.
- **Never background-poll** for device approval; wait for the user to confirm first.
- Respect `interval` if you must retry `DeviceToken` after a user re-confirmation; back off further on `SLOW_DOWN`.
- Once you hold a token, hand off to the `portal-api` skill for the actual calls;
  come back here only to refresh or re-login.
