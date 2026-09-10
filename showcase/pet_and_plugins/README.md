# ReadMD Desktop Pet & Plugin Center Showcase Assets
> Comprehensive visual evidence and media assets for ReadMD v2.3.8 Desktop Pet Companion, Apple-grade Settings, Modernized Plugin Center, and Multi-lingual i18n Localization.

---

## 1. Overview & Directory Structure

This folder contains verified, high-definition (Retina 2x, 60fps CRF 18 H.264) media assets recorded directly from real browser execution via Playwright (`ui-tests/record_pet_and_plugins.js`).

```
showcase/pet_and_plugins/
├── README.md                                   # This documentation
├── samples/                                    # Real test files used during walkthrough
│   ├── quantum_ai_notes.md                     # Markdown document for reading companion test
│   └── formula_analysis.tex                    # Academic LaTeX document for direct drop-to-convert test
├── snapshots/                                  # High-resolution Retina 2x screenshots
│   ├── 01-plugin-center-apple-view.png         # Modernized Apple HIG Plugin Management Center
│   ├── 02-pet-settings-apple-sheet.png         # Inset Grouped Desktop Pet Settings Sheet
│   ├── 03-pet-slider-interaction.png          # Real-time Scale & Opacity range slider interaction
│   ├── 04-pet-direct-manipulation-drag.png    # Direct pointer manipulation (setPointerCapture)
│   ├── 05-pet-interactive-bubble.png          # Interactive mascot bounce & frosted speech bubble
│   ├── 06-reading-companion-milestone.png     # Reading progress observer & encourage bubble
│   ├── 07-pet-drop-ring-active.png            # Direct file drop target pulsing blue ring
│   └── 08-pet-drop-convert-success.png        # Instantly converted document loaded in reader
└── videos/                                     # Recorded walkthrough videos (CRF 18, H.264)
    ├── 01-plugin-center-modernization.mp4      # Scene 1: Plugin Center Modernization & Status Check
    ├── 02-pet-settings-and-configuration.mp4   # Scene 2: Desktop Pet Settings & Appearance Configuration
    ├── 03-pet-drag-and-interaction.mp4        # Scene 3: Direct Manipulation & Interactive Companion
    ├── 04-reading-companion-scroll-progress.mp4# Scene 4: Reading Progress Companion
    ├── 05-pet-direct-drop-convert.mp4         # Scene 5: Direct File Drag & Drop to Convert
    └── 06-pet-and-plugins-master-walkthrough.mp4# Scene 6: Full Suite Master Walkthrough
```

---

## 2. Walkthrough Scenarios & Verifications

### 🎬 Scene 1: Modernized Plugin Management Center
- **Video**: `videos/01-plugin-center-modernization.mp4`
- **Key Snapshot**: `snapshots/01-plugin-center-apple-view.png`
- **Design Principles**: `/apple-design`, `/design-taste-frontend`, `/frontend-design`.
- **Highlights**:
  - Translucent frosted glass modal overlay (`backdrop-filter: blur(16px)`).
  - System status pill indicating sandbox readiness and zero global dependency pollution.
  - Clear, balanced Inset Cards with left-aligned toggle switches and right-aligned secondary actions.
  - Instant toggle response on pointer press without layout shifts.

### 🎬 Scene 2: Desktop Pet Settings & Real-Time Configuration
- **Video**: `videos/02-pet-settings-and-configuration.mp4`
- **Key Snapshots**: `snapshots/02-pet-settings-apple-sheet.png`, `snapshots/03-pet-slider-interaction.png`
- **Highlights**:
  - Apple Inset Grouped Settings Sheet (`.apple-grouped-list`).
  - Breathing mascot preview stage on soft radial blue aura with live state badge.
  - Native Apple-style range sliders (`#pet-scale`, `#pet-opacity`) with real-time tabular numeric feedback (`33% -> 48%`, `100% -> 85%`).
  - Dual-channel persistence syncing with both backend API and frontend live state.

### 🎬 Scene 3: Direct Manipulation & Interactive Mascot Companion
- **Video**: `videos/03-pet-drag-and-interaction.mp4`
- **Key Snapshots**: `snapshots/04-pet-direct-manipulation-drag.png`, `snapshots/05-pet-interactive-bubble.png`
- **Highlights**:
  - Direct pointer manipulation using `setPointerCapture` and grab offset tracking.
  - Continuous 1:1 viewport boundary clamping and automatic position persistence to `localStorage`.
  - Mascot interactive tap: triggers joyful bounce animation (`petJoyBounce`), waving animation (`hermes-waving`), and pops friendly Apple frosted speech bubble (`#pet-bubble`).

### 🎬 Scene 4: Reading Progress Companion
- **Video**: `videos/04-reading-companion-scroll-progress.mp4`
- **Key Snapshot**: `snapshots/06-reading-companion-milestone.png`
- **Highlights**:
  - Autonomous scroll position observer on document reading area.
  - Non-intrusive encouraging milestones at 25%, 50%, 80%, and 100% progress.
  - Auto-dismissing speech bubbles (4s timeout) that never obscure user reading focus.

### 🎬 Scene 5: Direct File Drag & Drop to Convert
- **Video**: `videos/05-pet-direct-drop-convert.mp4`
- **Key Snapshots**: `snapshots/07-pet-drop-ring-active.png`, `snapshots/08-pet-drop-convert-success.png`
- **Highlights**:
  - Dragging academic or media files directly over the floating pet activates the `.is-drop-target` pulsing dashed blue ring.
  - Immediate audio-visual feedback on drop: companion announces `"收到文件！正在为你开启极速转换..."`.
  - Seamlessly invokes conversion pipeline and displays freshly parsed Markdown inside reader.

### 🎬 Scene 6: Master Walkthrough
- **Video**: `videos/06-pet-and-plugins-master-walkthrough.mp4`
- **Highlights**: Seamless continuous demonstration encompassing all 5 scenarios above.

---

## 3. Reusability & Reproduction

To reproduce or re-record all showcase assets:
```bash
# Ensure UI server dependencies and Playwright are installed
cd ui-tests
node record_pet_and_plugins.js
```
The script will automatically start the background server, record all scenes, transcode with FFmpeg (CRF 18 H.264), take Retina screenshots, and synchronize assets.
