# Beanfun Next — UI Design Brief for Stitch

## Project Overview

**Beanfun** is a Windows desktop game launcher / account manager for Beanfun (a Taiwanese/Hong Kong gaming platform). It manages login, game service accounts, OTP (one-time password) retrieval, and game launching.

We are rewriting the existing WPF (.NET) app using **Tauri v2 + Vue 3 + TypeScript + Element Plus**. The frontend needs a complete **redesign** — modern, clean, and polished — while keeping all functionality identical.

## Tech Constraints

- **UI library**: Element Plus (Vue 3 component library based on Element UI)
- **Icons**: Element Plus built-in icons (`@element-plus/icons-vue`) or any icon set compatible with Vue 3
- **Theming**: The app supports **runtime theme color switching** via CSS variables (`--el-color-primary`). Default is `#FF8201` (orange). Users can pick from 8 presets (#FF8201, #B6DE8E, White, Black, LightBlue, Pink, Gold, Silver) or enter any custom hex. **All accent colors must derive from this single variable — never hardcode a specific accent color.**
- **i18n**: All visible text must use `{{ t('KeyName') }}` (vue-i18n). Do NOT hardcode Chinese text. I will provide the translation keys.
- **Desktop app**: Fixed window size (~480px wide for main window), not responsive. Dialogs are auto-sized to content.
- **Custom title bar**: The app uses `decorations: false` in Tauri so we need a custom title bar component with drag region.
- **Dark/Light**: Not required for v1. Light theme only (with Mica/Acrylic glassmorphism).

## Design Direction — Glassmorphism + Fluent + Soft Depth

The visual style combines **Windows Fluent Design** materials with **glassmorphism** panels and **gradient accent colors**. The goal is a modern, lightweight feel that looks native on Windows 10/11.

### Layer Model (back → front)

| Layer | Material | CSS Approximation |
|-------|----------|-------------------|
| **Window backdrop** | Tauri Mica — picks up the user's desktop wallpaper tint | Handled natively by Tauri `window-vibrancy: "mica"` — no CSS needed |
| **Content panels** | Acrylic frosted glass | `background: rgba(255,255,255,0.65); backdrop-filter: blur(30px) saturate(1.4);` |
| **Cards / List items** | Subtle frosted overlay on top of panel | `background: rgba(255,255,255,0.45); backdrop-filter: blur(12px);` |
| **Elevated elements** | Floating (dialogs, tooltips, dropdowns) | Same acrylic + stronger shadow |

### Key Visual Elements

- **Frosted-glass panels**: Semi-transparent white with blur. Each panel has a thin highlight border on top to simulate light: `border-top: 1px solid rgba(255,255,255,0.5);`
- **Soft multi-layer shadows**: Cards and panels use subtle compound shadows: `box-shadow: 0 2px 8px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.06);`
- **Rounded corners**: Panels `12px`, buttons `8px`, inputs `6px`, avatars `50%`
- **Underline-style inputs** (Fluent): Inputs use a bottom border only, no full box border. On focus, the bottom border transitions to theme color. The left icon (lock, beanfun logo) is inline.
- **Reveal Highlight on hover**: Title bar buttons and list items show a subtle radial light gradient that follows the mouse cursor (CSS `radial-gradient` positioned via JS/CSS `pointer`)
- **Gradient accent buttons**: Primary action buttons (Login, Get OTP) use a linear gradient of the theme color: e.g., `linear-gradient(135deg, lighten(primary, 10%), darken(primary, 8%))`. This gradient auto-derives from whichever theme color the user selects.
- **Selected list items**: Use the theme color as background with white text. The background is a subtle gradient, not a flat fill.
- **Game avatar glow**: The circular game icon on the login page has a soft glow shadow in the theme color: `box-shadow: 0 0 24px rgba(primary, 0.3);`
- **Page transitions**: Pages cross-fade with a slight vertical slide (`opacity 0→1` + `translateY(8px→0)` over 200ms ease-out)
- **Drag feedback**: When dragging an account list item, the dragged item scales up slightly (`scale(1.03)`) with an elevated shadow, and a thin theme-colored line indicates the drop position.

### What to Avoid

- Hard drop shadows (use only soft, diffuse shadows)
- Fully opaque backgrounds on panels (always use some transparency)
- Neon / glow effects (keep glow subtle and only on the game avatar and QR code frame)
- Heavy borders (prefer 1px subtle borders or no border at all)

### Reference Apps

- Windows 11 Settings app (Mica + Acrylic material layers)
- Windows Terminal (transparent background, Fluent inputs)
- Arc Browser (gradient accents, glassmorphism sidebar)
- Figma desktop app (soft depth, clean panels)
- Logi Options+ (multi-layer card depth)

### The Beanfun Logo

The Beanfun logo is a stylized character (SVG path data will be provided). It appears in the title bar left side, followed by the app name as a second SVG path.

---

## App Shell

### `AppShell` (Main Window)

The root container. Fixed size, non-resizable.

**Structure:**
- **Custom Title Bar** (top, 32px height, transparent background — Mica shows through):
  - Left: Beanfun logo (SVG) + App name text (SVG), both in `text-primary` color
  - Right: Icon buttons — `ℹ️ About` | `⚙️ Settings` | `Region label (TW/HK)` | `➖ Minimize` | `✕ Close`
  - Entire title bar area is draggable (except buttons)
  - Close button hover: `danger` red background (`#d44027`) with white icon
  - Other buttons hover: **Reveal Highlight** — a subtle `radial-gradient(circle at pointer, rgba(0,0,0,0.06), transparent 60%)` that follows the cursor
- **Content Area** (below title bar): Vue Router `<router-view>` renders pages here
- System tray icon support (minimize to tray is a setting)

---

## Pages (11 screens)

### Page 1: `IdPassForm` — Username & Password Login

This is the **primary login screen** and the first thing users see.

**Layout (3-column):**
- **Left sidebar** (~90px): Bottom-aligned "Register Account" text link
- **Center content**:
  - Game avatar image (86x86, circular with semi-transparent white border, **theme-color glow shadow**: `box-shadow: 0 0 24px rgba(var(--el-color-primary), 0.3)`) — clickable to open game selector
  - Account input: **Fluent underline-style** editable ComboBox (bottom border only, focus → theme-color bottom border with center-expand animation). Dropdown shows saved accounts, each with a `×` delete button. Placeholder: "Account or Email". Beanfun logo icon on the left, colored same as border.
  - Password input: **Fluent underline-style** password field with lock icon on left. Placeholder: "Password". Lock icon animates (changes to "unlocked" SVG path) on focus. Focus border transitions to theme color.
  - Row: `☑ Remember Password` | `☑ Auto Login` | `Forgot Password?` (link)
  - Row: `[Login]` button (**gradient accent**: `linear-gradient(135deg, lighten(primary,10%), darken(primary,8%))`, full width minus game-start) | `[Start Game]` button (secondary style, frosted glass background)
- **Right sidebar** (~90px): Bottom-aligned icons — QR Code login toggle button, GamePass login toggle button (conditionally visible)

**States:**
- Empty (no saved accounts)
- With saved accounts (dropdown shows list)
- Loading (after clicking login, buttons disabled, show spinner)
- Error (show ElMessage error toast)

**Interactions:**
- Clicking game avatar opens `GameList` dialog
- Selecting saved account fills password if "remember" was checked
- `×` in dropdown item deletes that saved account
- Login button triggers login flow → navigates to `LoginWait` → then `AccountList` on success
- QR button toggles to `QrForm`, GamePass button toggles to `GamepassForm`

---

### Page 2: `QrForm` — QR Code Login

**Layout (centered):**
- Acrylic frosted-glass panel background
- QR Code image (150x150, with subtle **theme-color glow frame**: `box-shadow: 0 0 20px rgba(var(--el-color-primary), 0.2)`) — clickable to refresh
  - Optionally shows a "Scan with Beanfun app" tip image on the right
- `[Copy Deeplink]` button (250px wide) — copies QR login URL to clipboard
- Row: `[Back to Regular Login]` button | `[Start Game]` button

**States:**
- Loading QR code (spinner)
- QR code displayed (waiting for scan)
- QR code expired (overlay with "Expired, click to refresh")
- Scan detected → auto-navigate to `LoginWait`

---

### Page 3: `GamepassForm` — GamePass Login

**Layout (centered, simple):**
- Status text: "Waiting for GamePass login..." (i18n)
- `[Open GamePass]` button (250px wide) — opens a separate WebView window for GamePass authentication
- `[Back to Regular Login]` button (250px wide)

**States:**
- Idle (waiting)
- GamePass window opened (status text changes)
- Login complete → auto-navigate to `AccountList`

---

### Page 4: `LoginTotp` — TOTP 6-Digit Input

**Layout (centered):**
- Semi-transparent background
- Label: "Enter TOTP verification code" (i18n)
- **6 individual digit input boxes** in a row, each accepts 1 character:
  - Large font (20px), centered text
  - Auto-focus next box on input
  - Support paste: pasting "123456" fills all 6 boxes
  - Boxes separated by ~10px gaps
- `[Login]` button
- `[Cancel]` button (smaller, below)

---

### Page 5: `LoginWait` — Login In Progress

**Layout (centered, minimal):**
- Semi-transparent white panel
- Loading indicator (spinner or animated dots)
- Text: "Logging in..." (i18n)
- `[Cancel]` button

---

### Page 6: `VerifyPage` — Advanced Verification

**Layout:**
- Semi-transparent white panel
- Row: Verification info input (TextBox, placeholder: "Enter verification info") + `☑ Remember` checkbox
- Row: "Your verification method:" label + verification type display (e.g., phone number partially masked)
- Captcha code input (TextBox, placeholder: "Enter captcha code")
- Captcha image (160x36, clickable to refresh, with tooltip "Click to refresh")
- `[Confirm]` button

---

### Page 7: `AccountList` — Main Dashboard (Post-Login)

This is the **most important screen** — users spend most of their time here.

**Layout:**
- **Top section** (game info bar):
  - Left: Game icon (48x48) + Game name button (clickable → opens `GameList`)
  - Below game name: `[Start Game]` button
  - Right: `[Logout]` button + `[Tools]` button (stacked vertically)
- **Menu bar**: `Gash Balance ▾` (submenu: Refresh / Recharge / App Recharge) | `Member Center` | `Customer Service`
- **Account list** (main area, scrollable, 290px+ wide, ~130px tall):
  - Each item: Account display name (left) + `≡` drag handle (right, grey, cursor: grab)
  - Selected item: highlighted with **theme-color gradient background**, white text (auto-switch to dark text for light theme colors like White/Silver)
  - Hover: **Reveal Highlight** — subtle radial gradient following cursor
  - **Double-click**: copies account ID (shows toast)
  - **Right-click context menu**: Copy Account / Change Name / Change Password / Account Info / — / Member Center / Customer Service / Check Email / — / Official Site
  - **Drag & drop**: reorder accounts (persisted)
  - Disabled accounts shown in grey with "Account Banned" tooltip
- **Account limit row**: Account count notice (left) + `[Add Service Account]` button (right)
- **OTP row**:
  - `[Get OTP]` button (primary, default action)
  - `☑ Auto Paste` checkbox (with tooltip explaining the feature)
  - OTP display TextBox (read-only, centered text, click to copy)

**States:**
- No accounts (empty list with prompt)
- Accounts loaded (normal)
- OTP retrieved (password field shows the OTP, auto-clears after timeout)
- Game running (Start Game button might change state)

---

### Page 8: `ManageAccount` — Local Account Management

**Layout:**
- Header: User icon (white, with shadow) + "Manage Accounts" title (large, white, with shadow) — **theme-color gradient background** (`linear-gradient(135deg, lighten(primary,5%), darken(primary,10%))`)
- White content panel:
  - **Region tabs**: `[Taiwan]` | `[Hong Kong]` — text-style toggle buttons (disabled = selected, shows in black)
  - **Account ListView** (table, 200px tall):
    - Columns: Account | Remark | Remember Password | Auto Login | Remember Auth Info
    - Single selection
  - **Action row**: `[Data Backup]` (left) | `[↑]` `[↓]` `[Add]` `[Edit]` `[Delete]` (right)
    - ↑↓/Edit/Delete buttons disabled when no selection
  - `[Back]` button

---

### Page 9: `Settings` — App Settings

**Layout:**
- Header: Gear icon (white, with shadow) + "Settings" title (large, white, with shadow) — **theme-color gradient background** (same as ManageAccount)
- Acrylic frosted-glass content panel, 2-column layout:
  - **Left column (controls)**:
    - `[Manage Accounts]` button
    - Update Channel: dropdown (Stable / Development)
    - Language: dropdown (populated dynamically)
    - Theme Color: editable dropdown with **color preview swatch** next to each option (presets: `#FF8201` Orange, `#B6DE8E` Green, `#FFFFFF` White, `#000000` Black, `#ADD8E6` LightBlue, `#FFC0CB` Pink, `#FFD700` Gold, `#C0C0C0` Silver + custom hex input). Changing this immediately updates all accent-derived colors across the entire app.
    - Login Mode: dropdown (Regular / QR Code)
  - **Right column (checkboxes)**:
    - ☑ Auto check for updates
    - ☑ Start game after login
    - ☑ Minimize to system tray
    - ☑ Disable hardware acceleration (with tooltip)
  - **Separator**: "Game" section header (grey text + horizontal line)
  - Game Path: label + readonly textbox (showing detected path)
  - **2-column checkboxes**:
    - Left: ☑ Traditional login mode (with tooltip) | ☑ Auto kill patcher (with tooltip)
    - Right: ☑ Skip play window (with tooltip) | `[Tools]` button
  - `[Back]` button

---

### Page 10: `About` — About Page

**Layout:**
- Top section (no background):
  - App icon (35px) + App name (bold, 20px) + "By Pungin" (author)
  - Version: "Version" label + version number + "Check Update" hyperlink
- White content panel:
  - About text (formatted, wrapped, 300px wide) — supports bold/links in the text
  - Separator line
  - Contact section: Email icon + email hyperlink | GitHub icon + "Github" hyperlink
  - `[Back]` button (centered, 100px wide)

---

### Page 11: `LoginPage` — Login Container

This is just a transparent container/frame that hosts the login sub-pages (`IdPassForm`, `QrForm`, `GamepassForm`, `LoginTotp`, `LoginWait`). It uses a nested `<router-view>` or `<component :is="">`. No visible UI of its own.

---

## Dialogs / Windows (18 screens)

All dialogs are **centered on screen**, auto-sized to content, using the **acrylic frosted-glass panel** style (`rgba(255,255,255,0.65)` + `blur(30px)`) with elevated shadow (`0 4px 16px rgba(0,0,0,0.08), 0 12px 32px rgba(0,0,0,0.1)`) and `border-radius: 12px`. Most are draggable by their title bar or content area.

### Dialog 1: `LoginRegionSelection` — Region Picker

**Simple centered dialog:**
- Beanfun logo + app name (same as title bar)
- Label: "Select Region" (i18n)
- Two large buttons side by side: `[Taiwan]` (120x50) | `[Hong Kong]` (120x50)
- Closes automatically on selection

---

### Dialog 2: `GameList` — Game Selector

**Grid dialog (wide, ~700px):**
- WrapPanel/Grid of game cards
- Each card: Game image (152x102) + Game name text below, with thin border
- Single-click selects and closes dialog
- Cards wrap to multiple rows

---

### Dialog 3: `AddAccount` — Add Local Account

**Form dialog:**
- Region dropdown (Taiwan / Hong Kong)
- Account input (placeholder: "Beanfun Account")
- Remark input (placeholder: "Remark")
- Password input (placeholder: "Password")
- Verification info input (placeholder: "Auth Info")
- Row: `☑ Auto Login` (left) | `[Add]` button (right)

---

### Dialog 4: `ChangeAccount` — Edit Local Account

**Form dialog:**
- Account input (placeholder: "Beanfun Account")
- Remark input (placeholder: "Remark")
- Row: `☑ Auto Login` (left) | `[Save]` button (right)

---

### Dialog 5: `AddServiceAccount` — Add Game Service Account

**Form dialog:**
- "Display Name" label + text input (170px)
- Checkbox: "I agree to the [Terms of Service]" (Terms is a hyperlink → opens `Contract` dialog)
- Row: `[OK]` (110px) | `[Cancel]` (110px)

---

### Dialog 6: `ChangeServiceAccountDisplayName` — Rename Game Account

**Form dialog:**
- "Display Name" label + text input (170px)
- Row: `[OK]` (110px) | `[Cancel]` (110px)

---

### Dialog 7: `ServiceAccountInfo` — Account Details

**Info display dialog:**
- Read-only fields (label: value format):
  - Account: (bold)
  - Serial Number: (bold)
  - Name: (bold)
  - Auth Type: (bold) — conditionally shown
  - Status: Normal (or other)
- "Account Established" section:
  - Small text: "Account established"
  - Large number (blue, 30px font): days count
  - Small text: "days"
  - Small red text: creation date
  - Small red text: last login date

---

### Dialog 8: `CopyBox` — Copy Text

**Minimal dialog:**
- Read-only text input (200px min) + `[Copy]` button
- Text is pre-filled with the value to copy

---

### Dialog 9: `CaptchaWnd` — Captcha Input

**Form dialog:**
- Captcha code input (placeholder: "Captcha Code")
- Captcha image (200x45, centered, clickable to refresh, tooltip: "Click to refresh")
- `[Confirm]` button

---

### Dialog 10: `Contract` — Terms of Service

**Read-only dialog (500x400):**
- Large scrollable text area showing terms of service text
- Read-only, no edit

---

### Dialog 11: `WebBrowser` — Embedded Browser

**Window (850x550):**
- Top: URL bar (read-only text input, showing current URL)
- Content: Full-size WebView area
- Used for: Member Center, Gash Recharge, Customer Service, Official Site, etc.

---

### Dialog 12: `AccRecovery` — Data Backup & Recovery

**Form dialog:**
- "Password" label + text input (200px)
- "Data" label + text input (200px)
- Row: `[Export]` (110px) | `[Recovery]` (110px)

---

### Dialog 13: `UnconnectedGame_AddAccount` — Add Game Account (Non-connected)

**Complex form dialog:**
- Instructional text with game name highlighted in green
- Detailed instructions about account creation rules
- Account ID input (with game name label)
- Nickname input (with placeholder, conditionally shown)
- Nickname instruction text
- Password input (with game name label)
- Confirm password input (with game name label)
- Side links: View terms | Check nickname availability
- Error message area (red, centered, conditionally shown)
- Row: `☑ I agree to [Game Name] terms` checkbox | `[Confirm]` button

---

### Dialog 14: `UnconnectedGame_ChangePassword` — Change Password (Non-connected)

**Simple form dialog:**
- "Verification Email" label + email input (200px)
- Error message area (red, centered, conditionally shown)
- `[Confirm]` button (centered)

---

### Dialog 15: `MapleTools` — MapleStory Toolbox

**Menu dialog (button list):**
- 5 buttons stacked vertically with margins:
  - Recycling (回收)
  - Player Report (舉報玩家)
  - Video Report (影片舉報)
  - Equip Star Force Calculator (裝備星力計算)
  - Perfect Core Calculator (完美核心計算)
- Each button opens a link in `WebBrowser` or opens a sub-dialog

---

### Dialog 16: `CoreCalculator` — Perfect Core Calculator

**2-panel window (654x404):**
- **Left panel (318px):**
  - "Required Skills" group box:
    - Skill name text input + `[Add]` / `[Delete]` buttons
    - Skills list (ListBox)
  - "My Cores" group box:
    - Main skill dropdown
    - Secondary skill dropdown × 2
    - `[Add]` / `[Delete]` buttons
    - Cores list (ListBox)
- **Right panel (316px):**
  - `[Calculate]` button (top)
  - "Results" group box: Read-only multiline text area showing calculation results

---

### Dialog 17: `EquipCalculator` — Star Force Calculator

**2-section window:**
- **Top section** (dark background):
  - Equipment Type: Radio buttons (Weapon / Glove / Armor / Accessory / Heart)
  - Heart notice (red text, shown only when Heart selected)
  - REQ LEV: Radio buttons (150 / 160 / 200)
  - Superior checkbox (green text, conditionally shown)
  - Stat row: "Stat +[total]" = (base + flame + star) — base & flame are editable, total & star-added are calculated
  - ATK/MATK row: same structure as stat
  - Star Force: input / max stars display
- **Bottom section** (light background):
  - 10 scroll types listed vertically, each with:
    - Scroll icon image (32px pixel art)
    - Scroll name label
    - Quantity input
    - Some scrolls have radio buttons (Min / Average / Max)
  - Last row: custom scroll stat input + ATK input
  - Scroll types: Destiny, Glory, Black, V, X, Red, Pinnacle, Speed, Legend, Other

---

### Dialog 18: `KartTools` — KartRider Toolbox

**Link menu dialog:**
- Section header: "Convoy Operations" (with grey separator line)
- 3-column layout of hyperlinks:
  - Column 1: Convoy Management | Convoy Ranking
  - Column 2: Convoy Search | Rider Search
  - Column 3: Create Convoy | Leave Convoy
- Each link opens a URL in `WebBrowser`

---

## Shared Components

### `TitleBar.vue`
- Height: 32px, **transparent background** (Mica shows through)
- Left: Logo SVG + App name SVG path (both in `text-primary` color)
- Right: icon buttons (About ℹ️ | Settings ⚙️ | Region text | Minimize ➖ | Close ✕)
- Draggable region: entire bar except buttons (`data-tauri-drag-region`)
- Button hover: **Reveal Highlight** (radial gradient following cursor)
- Close hover: `#d44027` red background + white icon
- Buttons are 28×28px (except Close which is 48×28px)

### `DraggableList.vue`
- Used in `AccountList` for drag-and-drop reorder
- Each item has a `≡` drag handle on the right (grey, `cursor: grab`)
- Visual feedback on drag: item **scales up** (`scale(1.03)`) with **elevated shadow** (`0 8px 32px rgba(0,0,0,0.12)`), slight rotation for natural feel
- Drop zone indicator: thin theme-colored line at insertion point

### `OtpInput.vue`
- 6 individual input boxes, each with **frosted-glass card background** and `border-radius: 8px`
- **Focus state**: bottom border transitions to theme color (Fluent underline)
- Auto-advance on type
- Auto-backspace on delete
- Support paste to fill all 6 boxes
- Large centered digits (20px font)

---

## Navigation Flow

```
App Start
  ├── First launch → LoginRegionSelection dialog → IdPassForm
  └── Has saved region → IdPassForm
  
IdPassForm
  ├── Click Login → LoginWait → (success) → AccountList
  │                           → (advance check) → VerifyPage → AccountList
  │                           → (TOTP required) → LoginTotp → LoginWait → AccountList
  │                           → (captcha required) → CaptchaWnd dialog → retry login
  │                           → (error) → show error message, stay
  ├── Click QR icon → QrForm
  ├── Click GamePass icon → GamepassForm
  ├── Click game avatar → GameList dialog → update game icon
  └── Click Register → open external URL

QrForm
  ├── Scan detected → LoginWait → AccountList
  └── Click Back → IdPassForm

GamepassForm
  ├── Click Open GamePass → Tauri WebView window → login complete → AccountList
  └── Click Back → IdPassForm

AccountList
  ├── Click game icon/name → GameList dialog
  ├── Click Start Game → launch game
  ├── Click Logout → IdPassForm
  ├── Click Tools → MapleTools/KartTools dialog (game-specific)
  ├── Click Get OTP → show OTP in text field
  ├── Double-click account → copy account ID
  ├── Right-click → context menu actions
  ├── Click Add Service Account → AddServiceAccount dialog
  ├── Gash menu → WebBrowser
  ├── Member Center → WebBrowser
  └── Customer Service → WebBrowser

Title Bar
  ├── About → About page
  ├── Settings → Settings page
  ├── Region → LoginRegionSelection dialog
  ├── Minimize → minimize window (or to tray based on setting)
  └── Close → close app

Settings
  ├── Manage Accounts → ManageAccount page
  ├── Tools → MapleTools/KartTools dialog
  └── Back → previous page

ManageAccount
  ├── Add → AddAccount dialog
  ├── Edit → ChangeAccount dialog
  ├── Data Backup → AccRecovery dialog
  └── Back → Settings
```

---

## Design Tokens

### Theme Color System (Runtime Switchable)

The **primary / accent color** is user-configurable at runtime. All accent-derived colors (button gradients, selected items, focus borders, glows) must be computed from a single CSS variable `--el-color-primary`. Never hardcode a specific accent color — always derive from the variable.

**Preset colors** (user picks one from a dropdown, or types a custom hex):

| Preset Name | Hex | Visual |
|-------------|-----|--------|
| Orange (default) | `#FF8201` | Warm, energetic |
| Green | `#B6DE8E` | Soft, natural |
| White | `#FFFFFF` | Minimal, clean (needs dark text buttons) |
| Black | `#000000` | Bold, high contrast |
| Light Blue | `#ADD8E6` | Cool, calm |
| Pink | `#FFC0CB` | Soft, playful |
| Gold | `#FFD700` | Rich, premium |
| Silver | `#C0C0C0` | Neutral, subtle |
| Custom | Any hex | User types e.g. `#6366F1` |

**Design must work with ALL of these colors.** This means:
- Button text on gradient background must auto-switch between white and dark based on contrast ratio
- The glow/shadow color on game avatar derives from the primary color with low opacity
- Selected list item uses the primary color; text must remain readable
- For very light primaries (White, Silver, Light Blue, Pink), selected items need a darker text or a border instead of relying on background alone
- Gradient buttons: `linear-gradient(135deg, color-mix(in srgb, var(--el-color-primary) 85%, white), color-mix(in srgb, var(--el-color-primary) 85%, black))`

### Fixed Tokens (Not User-Changeable)

| Token | Value | Usage |
|-------|-------|-------|
| Window Backdrop | Tauri Mica (native) | Desktop wallpaper tint bleed-through |
| Panel Background | `rgba(255, 255, 255, 0.65)` | Content panels, frosted glass |
| Panel Blur | `backdrop-filter: blur(30px) saturate(1.4)` | Frosted glass effect |
| Panel Highlight Border | `border-top: 1px solid rgba(255, 255, 255, 0.5)` | Top-light simulation |
| Card Background | `rgba(255, 255, 255, 0.45)` | Inner cards, list items |
| Card Blur | `backdrop-filter: blur(12px)` | Lighter blur for nested elements |
| Shadow (Panel) | `0 2px 8px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.06)` | Soft multi-layer |
| Shadow (Elevated) | `0 4px 16px rgba(0,0,0,0.08), 0 12px 32px rgba(0,0,0,0.1)` | Dialogs, dropdowns |
| Shadow (Drag) | `0 8px 32px rgba(0,0,0,0.12)` + `scale(1.03)` | Dragged items |
| Text Primary | `#1a1a1a` | Main text |
| Text Secondary | `#848484` | Placeholders, hints, secondary labels |
| Text Link Default | `#848484` | Unhovered links |
| Text Link Hover | `#484848` | Hovered links |
| Text Link Active | `#3AC3F7` | Clicked links, focused input accent |
| Danger | `#d44027` | Close button hover, error messages |
| Danger Hover BG | `rgba(212, 64, 39, 0.9)` | Close button hover fill |
| Success | `#67C23A` | Success states |
| Warning | `#E6A23C` | Warning states |
| Border Radius (Panel) | `12px` | Content panels, dialogs |
| Border Radius (Button) | `8px` | Buttons |
| Border Radius (Input) | `6px` | Input fields |
| Border Radius (Avatar) | `50%` | Game icon, user avatar |
| Title Bar Height | `32px` | Custom title bar |
| Main Window Width | `~480px` | Fixed width |
| Font Family | `"Segoe UI Variable", "Segoe UI", system-ui, sans-serif` | Windows native |
| Font Size (Body) | `14px` | Default text |
| Font Size (Small) | `12px` | Hints, secondary |
| Font Size (Header) | `30px` | Page titles (Settings, ManageAccount) |
| Transition Duration | `200ms` | Default transition speed |
| Transition Easing | `cubic-bezier(0.16, 1, 0.3, 1)` | Smooth ease-out |
| Page Transition | `opacity 0→1` + `translateY(8px→0)` over `200ms` | Page enter animation |

### Input Style (Fluent Underline)

```
Default:    border: none; border-bottom: 1px solid #d0d0d0; background: transparent;
Hover:      border-bottom-color: #a0a0a0;
Focus:      border-bottom: 2px solid var(--el-color-primary); (animated width expansion from center)
With icon:  Icon sits inline-start, vertically centered, colored same as border
```

---

## Important Notes

1. **All text uses i18n keys** — never hardcode Chinese. Use placeholder text like `{{ t('Login') }}`, `{{ t('Password') }}`, etc.
2. **Element Plus components** should be used wherever possible: `ElButton`, `ElInput`, `ElSelect`, `ElCheckbox`, `ElRadio`, `ElMessage`, `ElMessageBox`, `ElDialog`, `ElForm`, `ElFormItem`, `ElMenu`, `ElDropdown`, `ElTable`, `ElTooltip`, etc.
3. The app supports **3 languages**: Traditional Chinese (zh-TW), Simplified Chinese (zh-CN), English (en-US).
4. **No mobile/responsive design needed** — this is a fixed-size desktop app.
5. Dialogs should use `ElDialog` component from Element Plus, styled with the frosted-glass panel background and panel shadow.
6. Focus on making the **IdPassForm** and **AccountList** pages look exceptional — these are the two screens users interact with most.
7. **Theme color must be treated as a variable**, not a constant. Every accent-colored element (buttons, selected states, focus rings, avatar glows, gradient headers) derives from `var(--el-color-primary)`. Test your design mentally with at least Orange, Black, and White to ensure it works across the spectrum.
8. **Fluent underline inputs** replace traditional bordered inputs. Only the bottom border is visible; it animates to the theme color on focus with a center-expand effect.
9. Hover effects on title bar buttons and list items should use a **reveal highlight** (subtle radial gradient following the cursor).
