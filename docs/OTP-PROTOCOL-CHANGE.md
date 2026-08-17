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

## `LaunchTicket` — it is inside `Data`, not computed by GGM

Reverse engineering by @takidog on
[pungin/Beanfun#368](https://github.com/pungin/Beanfun/issues/368)
settles this. GGM does **not** derive `LaunchTicket`; it *decrypts* it
out of the `Data` field the web page already hands over. So the value
does reach us — obfuscated — and no binary needs to be reimplemented
to get it.

`GGMWebStart.Command.DecryptParam()`:

1. `n = int(data[0], 16)` — first character, as a hex digit.
2. Drop that character.
3. Pick substitution table `n % 4`.
4. Map each character to its index in the table, emitted as a hex
   digit — call the result the *normalized hex*.
5. The 8 characters at offset `n + 1` are the DES key (ASCII).
6. Remove those 8 characters; the remainder is the ciphertext hex.
7. DES-ECB decrypt, `Padding = None`.
8. UTF-8 decode, strip trailing `\0`.
9. Split on `;`, then parse `key=value` pairs joined by `&`.

The four hardcoded tables:

```text
0: bac987d65e432f10
1: 3bc4d5e6f2a79108
2: cdbeaf9012456378
3: 4e6fb81a3c5d7092
```

Each is a complete permutation of the 16 hex digits — verified, and a
prerequisite for step 4 to be well defined.

The sample in that issue is internally consistent: `len(Data) = 553`,
selector `5` → table 1, key offset 6, ciphertext 544 hex = 272 bytes
(a multiple of the 8-byte DES block), plaintext 266 bytes after
stripping six NULs.

Decrypted plaintext:

```text
LaunchTicket=<64 hex>
ServiceCode=<6>
ServiceRegion=<2>
ServiceAccount=<20>
BeanfunUrl=<URL>
WebStartPatch=<URL>
;<5 hex>
```

**Steps 5–8 are already implemented here.** `core::wcdes::decrypt_hex`
takes an 8-character ASCII key plus hex ciphertext and runs DES-ECB
with no padding — exactly this. It is the same construction the old
step 6 used (`payload.split_at(8)`), now wrapped in a substitution
layer. Only steps 1–4 and 9 are new code.

## What still needs GGM: `CV` and `Hash`

Correcting an earlier guess in this document — these are not cache
revalidation:

- `CV` = the **.NET assembly version** of `GGMWebStart.dll`
  (`Assembly.GetExecutingAssembly().GetName().Version`), not the PE
  file version and not the GameInfo.ini version it coincidentally
  matches in length.
- `Hash` = SHA-256 of `GGMWebStart.dll`'s bytes, lowercase hex,
  computed at runtime. Not a constant in the binary.
- `arch` = `Environment.Is64BitProcess` → `x64` / `x86`.

This pair is a **client-attestation gate**: the server is asking the
caller to prove it is running the official launcher. Sending it from
anything else is asserting an identity we do not have. It is also the
part most likely to be tightened later (per-build salting, obfuscation,
signing), so treat any automation around it as load-bearing but
fragile.

The upstream launcher path is:
`C:\Program Files\gamania Games\gamania Games Manager\GGMWebStart.exe`,
version 1.5.0.2.

## The `adapter.ashx` command codes

Static analysis of GGM gives the numeric `cmd` values behind the
five-character parameter observed in the capture:

```text
GET  /generic_handlers/adapter.ashx?cmd=01004&d=<TickCount>
GET  /generic_handlers/adapter.ashx?cmd=01003&service_code=…&service_region=…&d=…&CV=…&Hash=…&arch=…
GET  /generic_handlers/adapter.ashx?cmd=06002&sn=…&result=…&d=<TickCount>
POST /beanfun_block/generic_handlers/get_webstart_otp_v2.ashx
```

## Two ways forward

### A. No GGM — decode `Data` ourselves

Now the preferred route. We already fetch `game_start_step2.aspx` in
step 1, and `Data` is a page-supplied value, so it is likely already in
a response we hold; that needs confirming (see the open question).
Then: decode `LaunchTicket` per the algorithm above, and `POST` the v2
payload ourselves.

Remaining dependency is only `CV` + `Hash`, which are properties of a
GGM release rather than of the session — see "Keeping CV/Hash current".

### B. Delegate to GGM and intercept the OTP

Kept as a fallback. The OTP still reaches the game as **command-line
arguments**; the per-game GameInfo.ini gives the launch template:

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

## Keeping `CV` / `Hash` current

They change only when Gamania ships a new GGM — a handful of times a
year. `CheckVersion.ashx` returns `{ url, version }` in 80 bytes and is
the natural change detector.

Recommended shape: **detect in CI, ship in a release.** A scheduled job
polls `CheckVersion.ashx`; on a version change it downloads the
installer, extracts `GGMWebStart.dll` (`7z x`), computes the SHA-256,
reads the assembly version, and opens a PR bumping two constants. The
app carries them as constants — no runtime dependency, no extra request
on the OTP path, and every change is reviewable in git history.

The alternative floated upstream — publish the values to GitHub Pages
and have the launcher fetch them at runtime — trades a release cycle
for a permanent network dependency inside the auth path, gives every
user the same single point of failure with no rollback through the
normal release channel, and adds latency to each OTP fetch. For a value
that changes three times a year that is a poor trade. If runtime
refresh is wanted anyway, fetch with a fallback to the baked-in
constant, cache for a day, and never let it block the OTP call.

Two traps for the CI job:

- `CV` is the **managed assembly version**, which can differ from the
  PE `VERSIONINFO`. Read the .NET metadata, not just the file version.
- The four substitution tables live inside the DLL. Hash and version
  automate cleanly; extracting the tables does not. Treat an algorithm
  change as a manual review item — it should be rare, and it will
  announce itself as a decode failure.

## Where `Data` comes from — confirmed

`game_start_step2.aspx` — **the page step 1 already downloads**. So the
fix needs a new scrape, not a new request. The literal:

```javascript
var m_objData = {
    "region": "TW;Production",
    "sn": "<36-char GUID>",
    "data": "<537 characters>"
};

function LaunchGame() {
    if (supportService.indexOf(MyAccountData.ServiceCode) > -1) {
        parent.GGM.SmartLaunch(m_objData, …);   // Cmd=06006
    } else {
        parent.GGM.LaunchGame(m_objData, …);    // Cmd=06004
    }
}
```

Three things worth noting:

- `region` is the literal `TW;Production`, not `TW`.
- There is **no** `webToken` or `secretCode` property, so `ggm.js`
  takes its `else` branch and the handoff URI is only
  `Region&SN&Cmd&Data`. Those two values never reach the launcher.
- `supportService` is a hardcoded service-code allowlist that picks
  `SmartLaunch` over `LaunchGame`; `610074` (MapleStory TW) is in it.

The observed `data` length of 537 satisfies the decoder's arithmetic:
`537 - 1 - 8 = 528` hex characters = 264 bytes = 33 whole DES blocks.
The upstream sample's 553 gives 272 bytes = 34 blocks. Two independent
captures landing exactly on the block boundary is good evidence the
format is understood.

## Status

Implemented, **not yet run against a live server**:

1. `core::launch_data::decode_launch_ticket` — decodes the blob.
2. `parse_launch_handoff` — scrapes `m_objData` from step 1's response.
3. `GGM_CV` / `GGM_DLL_SHA256` / `GGM_ARCH` — pinned in
   `services::beanfun::otp`.
4. `step_5_post_otp_v2` — posts the payload and decrypts the reply's
   `data`.

`get_otp` selects the path on whether step 1's page carries
`m_objData`, so HK keeps the legacy endpoint and a later HK migration
needs no code change.

### The one unverified assumption

The reply's `data` is treated as the same `{8-char key}{ciphertext
hex}` envelope the pre-v2 protocol used. The observed length of 40 is
exactly 8 + 32 hex = 8 + two DES blocks, which is why — but no
plaintext OTP has been seen to confirm it. If the assumption is wrong
the symptom will be `auth.otp_decryption_failed`, or an OTP that comes
out as mojibake, rather than a silent wrong answer.

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
