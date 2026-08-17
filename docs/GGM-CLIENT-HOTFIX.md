# Hotfix runbook — TW OTP refused after a Game Manager release

TW password retrieval suddenly fails for many users at once, and the
Gamania Games Manager has just shipped a new build.

**No code change. No release.** Edit `ggm-client.json` at the repository
root and push.

You may arrive here from an issue labelled `ggm-version` instead of from
a user report — the watcher notices a new Game Manager on its own, which
is the point of it. Step 1 is still worth reading: a new build shipping
and users failing are two separate facts, and only the second one means
this lever is the fix.

---

## 1. Confirm it is actually this

Ask for a log: **Settings → Maintenance → open log folder**, newest file.
Look for:

```
Otp.Tw.V2  credential request refused  result=... message=...
```

`result != 1` with a message about a version or verification is the
symptom this lever fixes.

**Symptoms that are _not_ this** — changing values will not help, and
will cost you six hours if you get it wrong:

| In the log | What it means |
|---|---|
| `no usable launch ticket in the game-start page` | the page's shape changed → code change |
| `credential response was not JSON` | the response format changed → code change |
| `launch data: ...` decode errors | the decoding algorithm changed, e.g. new tables → code change |
| no `Otp.Tw.*` lines at all | this route was never taken; look elsewhere |

Users **with GGM installed are unaffected** — they read their own copy
and never consult the published file. So if every report comes from
someone without GGM, that is strong confirmation the published or
compiled-in pair has gone stale.

## 2. Get the new values

**Look for an open issue labelled `ggm-version` first.** The watcher
(`.github/workflows/ggm-watch.yml`) asks beanfun hourly which build it
ships, and when that moves it installs the thing on a Windows runner and
posts the finished document — both `cv` and `hash`, ready to paste. You
may well have the values before anyone reports a failure.

If there is no issue — the watcher failed, or beanfun raised the bar
without shipping a new build — run this on a Windows machine with the
**new** GGM installed:

```powershell
.\scripts\ggm-client.ps1
```

It prints `cv`, `hash` and `arch`, and assembles the document. No GGM?
Install it from <https://tw.beanfun.com/ggm/index.aspx>, or point at a
copy of the file with `-Dll <path>`.

Both halves have to come off the file itself. The version lives in a
Windows version resource, so a Linux runner can never read it — that is
the whole reason the watcher spends a Windows runner rather than
unpacking the installer.

To check the watcher by hand, or after fixing it: **Actions → Watch GGM
version → Run workflow**, with `force` ticked to make it install and
read even when the version matches.

## 3. Write it

```powershell
.\scripts\ggm-client.ps1 -Write
```

Editing by hand is fine, but save as **UTF-8 without BOM**. The app now
tolerates a BOM, so an editor that adds one will not break the fix — but
older builds in the field do not, so do not rely on it.

Validate before pushing:

```bash
python -c "import json;print(json.load(open('ggm-client.json')))"
```

`hash` must be **exactly 64 hex characters** and `cv` digits and dots
only. The app checks both and treats anything else as if the file were
absent, dropping silently to the next source.

## 4. Push to `code`

The published URL is read from the default branch:

```
https://raw.githubusercontent.com/pungin/Beanfun/code/ggm-client.json
```

Confirm it is live:

```bash
curl -s -o /dev/null -w "%{http_code}\n" \
  https://raw.githubusercontent.com/pungin/Beanfun/code/ggm-client.json
```

Expect `200`. The jsDelivr mirrors carry their own cache and may lag by
minutes; no need to wait for them.

## 5. When users get it

**Within six hours** — the local cache TTL. To take it immediately, a
user can delete `ggm-client.json` from `%APPDATA%\Beanfun\` and retry, or
install GGM, which bypasses this path entirely.

## 6. If you publish a bad pair

Everyone who was working breaks, because the published layer outranks
the compiled-in one.

- Revert the commit and push, or restore the last known-good values.
- **Users keep the bad values for up to six hours**, cache TTL. This is
  the expensive part, which is why step 7 is not optional.
- Emergency workaround for an individual: delete
  `%APPDATA%\Beanfun\ggm-client.json`.

## 7. Test before pushing

On a Windows machine with **no GGM installed**:

1. delete `%APPDATA%\Beanfun\ggm-client.json`
2. retrieve a password
3. the log should show `ggm-hotfix: published values fetched cv=<new>`,
   then the retrieval succeeding

To try values on a branch first, point the first entry of `HOTFIX_URLS`
in `services/beanfun/ggm_hotfix.rs` at that branch, then change it back.

## Things worth remembering

- Users with GGM installed never touch the network for this. That
  protects them from a bad publish — and means they cannot confirm a
  good one for you.
- We have **never actually seen beanfun refuse an old CV/Hash**. This is
  preventative. The first time it is really needed, take the extra
  minute on step 1 rather than swapping values because something broke.
- Swapping values cannot fix "the page changed" or "the response format
  changed". Those need code; the TW section of
  `services/beanfun/otp.rs` is the place to start.

## The order the app resolves in

1. **Pinned** — `%APPDATA%\Beanfun\ggm-client.json` with `"override": true`.
   A deliberate choice; nothing overrides it.
2. **Installed GGM** — read from `GGMWebStart.dll` on this machine. Self-updating,
   so it survives beanfun raising the bar with no action from us.
3. **Published** — this file, cached for six hours. The hotfix lever.
4. **Compiled in** — ships with the app, so a machine with none of the above works.
