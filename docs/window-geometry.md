# Window geometry contract

## Approved behaviour

The main window remembers the last position chosen by the user. A page or
announcement may require a different size. The window stays at the preferred
position when it fits, and otherwise moves only as far as necessary into the
selected monitor's working area, with a ten-logical-pixel gap on each corrected
edge. A single overflowing axis changes only that axis; two overflowing axes
produce equal adjacent gaps at the corner. Already fitting coordinates are not
snapped to the margin. Automatic corrections never replace the saved preference.
Returning to smaller content restores that preference where possible.

GamePass, reCAPTCHA, Classic login and the in-app browser are independent popup
windows. Each new popup is centred in the working area of the main window's
monitor. Popup positions are neither restored nor persisted. Users may move and
resize an open popup. Revealing an existing hidden popup checks its bounds without
discarding a position chosen during that opening.

Element Plus dialogues, announcements and QR overlays remain inside their host
window. They do not become independent native windows. Existing page dimensions,
dialogue layouts, popup minimum sizes and frontend zoom policy are preserved;
this change does not introduce responsive rearrangement or additional shrinking.

## Ownership and coordinates

- A Rust `WindowGeometryManager` owns native placement, working-area selection,
  bounds, user-position persistence and native lifecycle integration.
- Native application uses physical outer rectangles, including decorations.
  Edge gaps are measured from the visible frame, excluding invisible resize
  borders. Insets are converted using the destination monitor's scale factor;
  physical-pixel rounding may introduce at most one pixel of error.
  Requested content sizes use logical pixels and are converted using the target
  monitor's scale factor. Negative monitor coordinates are valid.
- The main frontend controller owns content measurement, serialised size/zoom
  requests and temporary overrides. It submits `resize_main_window(width, height)`
  rather than applying native sizes directly.
- Only the main webview may invoke that command. Remote pages receive no new IPC
  privileges.
- The browser's two child webviews retain their parent-relative layout. They are
  not independently positioned against a monitor.
- The monitor working-area origin and extent exclude space reserved by taskbars
  and registered appbars on any edge. Unregistered floating docks and a revealed
  auto-hide overlay may cover that area; the manager does not claim to detect
  arbitrary overlapping application windows.

## Complete inventory

Native initial content sizes (logical pixels):

- Main: 560 by 480, subsequently measured from its route.
- GamePass: 900 by 700.
- reCAPTCHA: 600 by 720; nominal minimum 520 by 600.
- Classic: 1024 by 720; nominal minimum 400 by 300.
- Browser: 850 by 596; nominal minimum 640 by 400. Its bar and content are
  separate child webviews. Requested bar heights are 46, 72, 300 and 326.

Main routes: region selection, ID/password, QR, GamePass, reCAPTCHA, TOTP,
waiting, verification, accounts, settings, about and account management. Their
design widths are respectively 420, 420, 420, 420, 420, 420, 420, 420, 480, 660,
420 and 880. Heights are measured after layout; obsolete fixed-height metadata
must not become the acceptance criterion. The browser-bar route does not own a
top-level content-fit window.

All sixteen in-window dialogues: AccRecovery, AddAccount, AddServiceAccount,
ChangeAccount, ChangeServiceAccountDisplayName, ClassicAccountPicker, Contract,
CopyBox, CoreCalculator, EquipCalculator, GameList, KartTools, MapleTools,
ServiceAccountInfo, UnconnectedGame_AddAccount and
UnconnectedGame_ChangePassword. Additional surfaces include the announcement,
announcement archive/banner, enlarged QR code, saved-account dropdown,
MessageBox, selectors, tooltips and browser connection panel.

System file pickers and externally launched applications retain OS ownership.
Authentication, game-launching and account-storage behaviour are outside this
change.

## Required invariants

1. An already fitting main rectangle is not moved. An overflowing visible edge
   is corrected to a ten-logical-pixel inset. The other axis remains unchanged
   unless it also overflows. Corrected corner gaps are equal after DPI conversion.
2. Placement preserves the requested dimensions and existing popup minima. A
   rectangle larger than the working area cannot be made fully visible by
   translation; verification must identify that limitation instead of reporting
   a successful full-containment assertion or silently shrinking the content.
   If the rectangle fits but leaves less than ten logical pixels spare on an
   axis, only the available gap can be used without changing its dimensions.
3. Monitor selection is stable during an apply operation. Missing monitors use an
   available nearest monitor. Work-area origins and taskbar reservations count.
4. Automatic resize, scale changes, startup restoration and tray restoration do
   not persist a replacement for the user's preferred main position. Explicit
   user dragging and the reset-position action may update it.
5. Existing main position data is migrated without deleting the original state.
   Popup entries in old state are ignored.
6. A new popup is centred after its final size is known. Re-showing an
   existing popup preserves user placement where it remains valid.
7. Size/zoom operations are serialised. Pending requests coalesce to the latest
   route and active override; only successful applications update cached state.
   A failed request remains retryable without an uncontrolled retry loop.
8. Temporary overrides are nestable. Releasing one resolves the latest remaining
   demand, rather than restoring a stale physical size from a different route or
   DPI setting.
9. Resize, DPI and observation subscriptions have bounded lifetimes. Route opt-out
   stops content fitting. The existing paint scheduler timeout and one-pixel
   convergence tolerance remain effective.
10. Existing frontend size and accessibility scaling calculations are retained.
    A change of width is followed by layout measurement at the resulting width.
    Announcement sizing retains its existing logical dimensions and activation.
11. Main-only overlays are not mounted in the browser toolbar webview. Internal
    dialogue sizes and layouts remain unchanged.

## Automated verification

Write failing behavioural tests before each implementation. A size is a width
and height pair. For each finite catalogue of N states, exercise all N(N-1)
directed transitions, N unchanged-size cases and A-to-B-to-A restoration.
Different routes or native window lifecycles remain separate cases even when
their dimensions match. Dynamic content produces additional measured states;
fixed route-width metadata is not an exhaustive catalogue of sizes.

Cross the geometry catalogue with all four edges, all four corners, an interior
position, all four taskbar reservations, negative monitor origins, asymmetric
invisible frame borders and scale factors 1, 1.25, 1.5 and 2. Verify dimensions,
one-axis corrections, equal corrected corner gaps, idempotence and unchanged
saved preferences. Include oversized requests and invalid dimensions explicitly.

Controller tests cover delayed requests, failure/retry, rapid navigation, nested
overrides and opt-out. Lifecycle tests cover manual dragging, cancelled dragging,
restart restoration and newly created versus re-shown popups. Component tests
cover host ownership. Run the frontend and Rust regression suites and repository
static checks after integration.

Real application checks are performed separately and reported in the conversation;
their results are not added to a test document. Mocked monitor data is not evidence
that a physical multi-monitor or DPI configuration has been exercised.
