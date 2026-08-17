# OTP protocol change — `get_webstart_otp.ashx` is dead

**Status**: investigated, not yet fixed. Captured 2026-08-17 against `tw.beanfun.com`.

`commands::get_otp` fails with `auth.otp_server_rejected`. The server
returns:

```
0;        Query String Error
```

(28 bytes: `0;`, eight spaces, then the message — the server reuses the
success envelope's 8-character key slot, blanked, for errors.)

Nothing on our side is malformed. Beanfun replaced the endpoint.

## Root cause

`services::beanfun::otp` is a 1:1 port of the WPF client's OTP chain.
Step 5 of that chain no longer exists in the form we call it:

| | what we send | what the official client sends |
|---|---|---|
| endpoint | `generic_handlers/get_webstart_otp.ashx` | `generic_handlers/get_webstart_otp_v2.ashx` |
| method | `GET` | `POST` |
| params | 9 query-string fields | JSON body |
| response | `1;{key}{cipher_hex}` text | `application/json` |

New contract:

```
POST /beanfun_block/generic_handlers/get_webstart_otp_v2.ashx
Content-Type: application/json

{ "SN": <36>, "LaunchTicket": <64>, "CV": <7>, "Hash": <64>, "arch": <3> }

→ { "result": 1, "data": <40>, "message": null }
```

Of our nine parameters only `SN` survives. `WebToken`, `SecretCode`,
`ServiceCode`, `ServiceRegion`, `ServiceAccount`, `CreateTime`, `d` and
the mystery `ppppp` constant are all gone. The OTP now arrives in
`data` (40 characters) instead of a DES-ECB envelope.

## The real change: the browser no longer fetches the OTP at all

`beanfun_block/game_zone/scripts/ggm.js` hands off to a locally
installed native helper (GGM — 遊戲橘子遊戲管理員, `GGMSetup_1.5.0.2.exe`,
17 MB) through a custom URL scheme:

```
gamaniagames://Region={0}&&&&SN={1}&&&&Cmd=06004&&&&WebToken={2}&&&&SecretCode={3}&&&&Data={4}
```

| `Cmd` | ggm.js function | carries WebToken/SecretCode |
|---|---|---|
| `06003` | `OpenSelect` | no |
| `06004` | `LaunchGame` | **yes** |
| `06005` | `OpenDownloader` | no |
| `06006` | `SmartLaunch` | **yes** |

Both token-carrying variants fall back to a `Data`-only form when
`webToken`/`secretCode` are absent.

GGM then performs the rest itself (all observed in the capture):

1. `generic_handlers/CheckVersion.ashx` → `{ url, version }`
2. `generic_handlers/adapter.ashx?cmd&d` → full GameInfo.ini (utf-16, ~25 KB)
3. `generic_handlers/adapter.ashx?cmd&service_code&service_region&d&CV&Hash&arch`
   → per-game GameInfo.ini (utf-16, ~2 KB)
4. `generic_handlers/adapter.ashx?cmd&sn&result&d` → empty
5. `POST get_webstart_otp_v2.ashx` → the OTP
6. launches the game with the OTP on its command line

So `WebToken` and `SecretCode` — the two values we currently put in
step 5's query string — are now *inputs handed to GGM*, not parameters
of the OTP call.

`CV` and `Hash` are **not** an integrity gate: the per-game
GameInfo.ini declares `Version=2.1.1.2`, exactly the 7 characters sent
as `CV`, and `Hash` is 64 hex. They are cache revalidation for that
file, and we could compute them ourselves.

## Blocker: `LaunchTicket`

`LaunchTicket` (64 characters) is derived inside GGM from
`WebToken` / `SecretCode` / `SN`. It is never transmitted — every
response in a 665-entry capture was checked and none contains a
64-character value. Reproducing it means reverse-engineering the GGM
binary.

## Chosen approach: let GGM fetch the OTP, intercept the result

The OTP still reaches the game as **command-line arguments**. The
per-game GameInfo.ini gives the launch template:

```
exe=MapleStory.exe tw.login.maplestory.beanfun.com 8484 BeanFun %s %s
```

The two `%s` are the account and the OTP. A long-standing community
trick (documented since 2013) exploits exactly this: replace the game
executable with a shim that prints its own `argv`. In C# `args`
(program name excluded) that is `args[3]` = account, `args[4]` = OTP —
which independently confirms the template above.

The plan, which avoids reverse-engineering `LaunchTicket` while keeping
the "show / copy / auto-paste OTP" feature:

1. Log in as we already do; keep `region`, `sn`, `webToken`, `secretCode`.
2. Invoke `gamaniagames://…&Cmd=06004&…` so the *official* GGM runs the
   `LaunchTicket` + `get_webstart_otp_v2` sequence.
3. Point Beanfun's configured game path at a shim executable we ship.
4. The shim receives `argv[3]` / `argv[4]` and hands them back to the
   main app.
5. The app displays / copies / auto-pastes as before.

Cost: GGM must be installed, and we gain a second executable plus a
path-configuration step.

Benefit: protocol-independent. Whatever Gamania changes server-side,
the OTP still ends up on that command line — which is why the 2013
trick still works today.

## Open question — blocks implementation

**What goes in `Data={4}`?** `ggm.js` only defines the interface; the
call site lives in an authenticated page (`loader.ashx` returns
`_bf_IsInitOK = false` without a session). The URI never crosses the
network, so it is not in any capture. Until its format is known, the
URI builder cannot be written.

To recover it, from a logged-in session: read the JS that calls
`ggm.LaunchGame(...)` / `ggm.SmartLaunch(...)` on
`beanfun_block/game_zone/game_start_step2.aspx`, and quote how the
object's `data` property is built.

## What has already been fixed

- `b2647bb` — the scrapers no longer forward empty captures, so an
  upstream failure is reported at its own step instead of surfacing as
  a generic step 5 rejection. Real bug, unrelated to this one.
- `b90237c` — the seven `errors.auth.otp_*` codes are localized; they
  used to fall through to the raw English backend message.
- `9fd548e` — step 5 failures log the verbatim server response and a
  redacted request URL, which is how this was diagnosed.

## Evidence

Fiddler Classic capture of the official client, 665 entries,
summarized with a redacting script (parameter names and value lengths
only). Key entries: `get_webstart_otp_v2.ashx`, the three
`adapter.ashx` calls, `CheckVersion.ashx`, and `ggm.js`.
