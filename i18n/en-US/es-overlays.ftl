## Overlay surfaces: region selector, countdown, capture deck, pinned screenshots, capture bar,
## recording options and bar, scroll-capture HUD. Shortcuts are passed in as { $shortcut }.

## Region selection overlay
region-size = { $w } × { $h }
region-adjust = { $w } × { $h }  ·  { $shortcut } to capture  ·  { $hint }  ·  esc
region-hint-drag-to-adjust = drag to adjust
region-hint-drag-edges = drag the edges to resize, or draw a new area

## Countdown
countdown-a11y =
    { $count ->
        [one] Starting in { $count } second
       *[other] Starting in { $count } seconds
    }

## Capture deck
deck-save-all = Save All
deck-clear-all = Clear All
deck-tool-pin = Pin
deck-tool-dismiss = Dismiss
deck-tool-copy = Copy
deck-tool-save = Save
deck-tool-edit = Edit
deck-tool-cloud-share = Cloud Share
deck-tool-show-in-folder = Show in Folder
deck-tool-preview = Preview
deck-tool-delete = Delete
deck-card-a11y-screenshot = Screenshot preview
deck-card-a11y-recording = Recording preview
deck-card-loading = Loading preview…
deck-card-saving = Saving…
deck-window-title = Capture overlay
deck-stack-count = { $count }
deck-stack-a11y = { $count } captures

## Pinned screenshot context menu
pin-copy-image = Copy Image
pin-close = Close
pin-a11y = Pinned screenshot

## Capture bar (picker mode)
bar-a11y = Capture bar
bar-caption-area = Area
bar-caption-screen = Fullscreen
bar-caption-window = Window
bar-caption-scroll = Scroll
bar-caption-text = OCR
bar-caption-color = Color
bar-caption-timer = Timer
bar-caption-recording = Recording
bar-title-area = Area
bar-title-screen = Screen
bar-title-window = Window
bar-title-scrolling = Scrolling Capture
bar-title-text = Copy text from screen
bar-title-color = Color
bar-recording-help = Recording options
bar-recording-missing = No screen recorder is installed. Install gpu-screen-recorder or wf-recorder.
bar-close-help = Close
bar-close-a11y = Close capture bar — Esc
bar-timer-a11y = Screenshot and recording timers
bar-timer-section-screenshot = Screenshot timer
bar-timer-section-recording = Recording timer
bar-timer-screenshot-off = Off
bar-timer-screenshot-seconds = { $seconds }s
bar-timer-recording-none = None
bar-timer-recording-seconds =
    { $count ->
        [one] { $count } second
       *[other] { $count } seconds
    }

## Recording options popover
rec-options-title = Recording
rec-options-subtitle = Choose a source to start recording.
rec-caption-display = Display
rec-caption-window = Window
rec-caption-area = Area
rec-caption-mic = Mic
rec-caption-audio = Audio
rec-tip-display-single = Record the whole screen
rec-tip-display-multi = Pick a screen to record
rec-tip-window = Pick an app window
rec-tip-area = Drag to select a region
rec-tip-mic-on = Microphone on
rec-tip-mic-off = Microphone off
rec-tip-audio-on = System audio on
rec-tip-audio-off = System audio off
rec-window-a11y = { $title } — { $app }
rec-window-untitled = Untitled window
rec-windows-workspace = Workspace { $name }
rec-windows-workspace-shown = Workspace { $name } · on screen
rec-windows-empty = No app windows found
rec-windows-refresh = Refresh Windows
rec-display-item = { $name } ({ $w }x{ $h })
rec-display-size = { $w } × { $h }
rec-display-fallback = Display { $n }
rec-mic-off = Off
rec-delay-label = Start delay
rec-delay-value = { $seconds } s
rec-delay-a11y = Start delay, { $seconds } seconds
rec-delay-less = Shorter start delay
rec-delay-more = Longer start delay
rec-a11y-display-single = Display - record the whole screen
rec-a11y-display-multi = Display - choose which screen to record
rec-a11y-window = Window - choose an app window to record
rec-a11y-area = Area - drag to select the region to record
rec-a11y-mic-off = Microphone off - click to choose an input
rec-a11y-mic-on = Microphone on - { $name }
rec-a11y-audio-on = System audio on - click to stop capturing what you hear
rec-a11y-audio-off = System audio off - click to capture what you hear

## Recording session bar
rec-bar-stop = Stop
rec-bar-preparing = Preparing
rec-bar-saving = Saving
rec-bar-paused = Paused
rec-bar-pause-help = Pause recording
rec-bar-resume-help = Resume recording
rec-bar-start-over-help = Start over
rec-bar-discard-help = Discard recording
rec-bar-stop-a11y = Stop and save recording, { $time } elapsed
rec-bar-a11y = Recording controls
rec-bar-stop-help = Stop and save
rec-bar-move-help = Drag to move
rec-bar-mute-mic-help = Mute microphone
rec-bar-unmute-mic-help = Unmute microphone
rec-bar-mute-system-help = Mute system sound
rec-bar-unmute-system-help = Unmute system sound
rec-confirm-restart-title = Start a new recording?
rec-confirm-restart-message = This recording will be discarded and recording will start again.
rec-confirm-restart = Start Over
rec-confirm-discard-title = Discard this recording?
rec-confirm-discard-message = This recording will be deleted without saving.
rec-confirm-discard = Discard
rec-confirm-cancel = Cancel
rec-tray-notice = Recording — stop from the tray or { $shortcut }

## Scroll capture HUD
scroll-title = Scroll Capture
scroll-title-size = Scroll Capture  ·  { $w }×{ $h }
scroll-auto = Auto Scroll
scroll-scrolling = Scrolling…
scroll-stop = Stop
scroll-cancel-a11y = Cancel Scrolling Capture
scroll-pause-auto-a11y = Pause Automatic Scrolling
scroll-start-auto-a11y = Start Automatic Scrolling
scroll-stop-a11y = Stop Scrolling Capture
scroll-hud-a11y = Scroll Capture, { $w } by { $h } points
region-a11y = Select an area to capture. Drag to draw it, Return to capture, Escape to cancel.

## Window mode
window-a11y = Choose a window to capture. Click a window, Space to select an area instead, Escape to cancel.
window-item = { $app } — { $title }
window-untitled = Untitled window
window-picker-title = Choose a window
window-picker-empty = No app windows found
window-picker-no-match = No windows match
window-picker-filter = Windows matching “{ $query }”
window-picker-hint = type to filter  ·  ↑ ↓ to choose  ·  ↩ to capture  ·  space for an area  ·  esc

## Colour picker
color-a11y = Pick a colour. Click to copy its hex value, Escape to cancel.

## Trim player
player-a11y = Video trimmer
player-video-a11y = Video
player-play = Play
player-pause = Pause
player-time = { $position } / { $duration }
player-timeline-a11y = Timeline
player-trim-start-a11y = Trim start
player-trim-end-a11y = Trim end
player-kept = { $start } – { $end }  ·  { $kept } s
player-mode-a11y = Export method
player-mode-precise = Precise
player-mode-fast = Fast
player-remove-audio = Remove Audio
player-export = Export
player-open-studio = Open in Studio
player-export-a11y = Export the trimmed video
player-share = Share
player-share-a11y = Upload the recording and copy a share link
player-exporting = Exporting… { $percent }%
player-exported = Saved { $name }
player-export-failed = Export failed: { $reason }
player-options-a11y = Export options
player-format = Format
player-format-a11y = Export format
player-format-mp4 = MP4
player-format-webm = WebM
player-format-gif = GIF
player-size = Size
player-size-a11y = Export size
player-size-original = Original
player-size-1080 = 1080p
player-size-720 = 720p
player-size-480 = 480p
player-quality = Quality
player-quality-a11y = Export quality
player-quality-high = High
player-quality-balanced = Balanced
player-quality-small = Small
player-look = Colour
player-look-a11y = Export colour
player-look-natural = Natural
player-look-vivid = Vivid
player-look-muted = Muted
player-look-mono = Mono
player-brightness = Brightness
player-volume-system = System sound
player-volume-mic = Microphone
player-open-failed = This video cannot be played: { $reason }
player-missing-plugins = GStreamer plugins are missing. Install { $packages } (Arch: sudo pacman -S { $packages }).

consent-title = A program wants to use easyshot
consent-program = { $program } (process { $pid })
consent-program-unnamed = Process { $pid }
consent-program-unknown = An unknown program
consent-wants-outputs = wants the list of your screens.
consent-wants-windows = wants the list of your open windows.
consent-wants-capture = wants to capture { $target }.
consent-wants-route = wants to run { $route }.
consent-wants-route-target = wants to run { $route } on { $target }.
consent-note-remember = While allowed, programs you run can do this without asking. Revoke it from the tray or Settings › Agents.
consent-note-once = Allowing lets this one request through.
consent-deny = Deny
consent-allow-hour = Allow for 1 hour
consent-allow-session = Allow until logout
consent-allow-always = Always
consent-allow-once = Allow
consent-change-on-title = Turn on agent access?
consent-change-always-title = Let agents work without asking?
consent-change-scope-title = Let agents capture more?
consent-change-on = asked to turn on agent access from the command line.
consent-change-always = asked that agents never be asked about again.
consent-change-scope = asked to widen what agents may capture.
consent-change-secrets-title = Show secrets to agents?
consent-change-log-title = Keep a shorter agent log?
consent-change-secrets = asked to stop blurring secrets in what agents capture.
consent-change-log = asked to keep fewer entries in the agent log.
consent-change-note = Nothing changes unless you allow it.
consent-allow = Allow
consent-cancel = Cancel
deck-card-shared-a11y = Shared
