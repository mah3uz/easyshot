## Overlay surfaces: region selector, countdown, capture deck, pinned screenshots, capture bar,
## recording options and bar, scroll-capture HUD. Shortcuts are passed in as { $shortcut }.

-app-name = easyshot

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
deck-card-a11y-screenshot = Screenshot preview
deck-card-a11y-recording = Recording preview
deck-card-open-image-editor-a11y = Open image editor
deck-card-open-recording-editor-a11y = Open recording editor
deck-card-loading = Loading preview…
deck-card-open-image = Open image
deck-card-open-recording = Open recording
deck-card-saving = Saving…
deck-window-title = Capture overlay
deck-share-off-title = Uploads are off
deck-share-off-message = Turn on Upload when I share in Settings → Sharing, then try again.
deck-share-unconfigured-title = Set up cloud sharing
deck-share-unconfigured-message = Add your cloud account in Settings → Sharing, then try again.
deck-share-prepare-title = Couldn’t prepare capture
deck-share-prepare-message = The screenshot is still in the deck. Check available disk space and retry.
deck-share-upload-failed-title = Upload failed

## Pinned screenshot context menu
pin-copy-image = Copy Image
pin-copy-video = Copy Video
pin-close = Close

## Capture bar (picker mode)
bar-caption-area = Area
bar-caption-screen = Screen
bar-caption-window = Window
bar-caption-scroll = Scroll
bar-caption-text = Text
bar-caption-color = Color
bar-caption-timer = Timer
bar-caption-recording = Recording
bar-title-area = Area
bar-title-screen = Screen
bar-title-window = Window
bar-title-scrolling = Scrolling Capture
bar-title-text = Copy text from screen
bar-title-color = Color
bar-timer-off-help = Timer off
bar-timer-recording-help = Recording timer { $seconds }s
bar-recording-help = Recording options
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
rec-options-loading = Loading recording sources…
rec-options-restart-permission = Restart { -app-name } so macOS applies its Screen Recording permission, then try again.
rec-caption-display = Display
rec-caption-window = Window
rec-caption-area = Area
rec-caption-camera = Camera
rec-caption-mic = Mic
rec-caption-audio = Audio
rec-caption-script = Script
rec-tip-display-single = Record the whole screen
rec-tip-display-multi = Pick a screen to record
rec-tip-window = Pick an app window
rec-tip-area = Drag to select a region
rec-tip-camera-on = Camera on
rec-tip-camera-off = Camera off
rec-tip-mic-on = Microphone on
rec-tip-mic-off = Microphone off
rec-tip-mic-unavailable = Microphone unavailable
rec-tip-mic-unsupported = Microphone requires macOS 15 or later
rec-tip-audio-on = System audio on
rec-tip-audio-off = System audio off
rec-tip-script-on = Teleprompter on
rec-tip-script-off = Teleprompter off
rec-window-item = { $app } - { $title }
rec-window-item-app-only = { $app }
rec-windows-empty = No app windows found
rec-windows-refresh = Refresh Windows
rec-display-item = { $name } ({ $w }x{ $h })
rec-display-fallback = Display { $n }
rec-mic-off = Off
rec-a11y-display-single = Display - record the whole screen
rec-a11y-display-multi = Display - choose which screen to record
rec-a11y-window = Window - choose an app window to record
rec-a11y-area = Area - drag to select the region to record
rec-a11y-camera-off = Camera off - click to record your camera, right-click to pick one
rec-a11y-camera-unavailable = Camera unavailable - right-click to choose another camera
rec-a11y-camera-on = Camera on - { $name }, right-click to switch
rec-a11y-mic-off = Microphone off - click to choose an input
rec-a11y-mic-unavailable = Microphone unavailable - choose another input
rec-a11y-mic-on = Microphone on - { $name }
rec-a11y-audio-on = System audio on - click to stop capturing what you hear
rec-a11y-audio-off = System audio off - click to capture what you hear
rec-a11y-script-on = Teleprompter on - click to edit the script
rec-a11y-script-off = Teleprompter off - click to write a script

## Recording session bar
rec-bar-stop = Stop
rec-bar-preparing = Preparing
rec-bar-saving = Saving
rec-bar-paused = Paused
rec-bar-pause-help = Pause recording
rec-bar-resume-help = Resume recording
rec-bar-start-over-help = Start over
rec-bar-discard-help = Discard recording
rec-bar-stop-help = Stop and save the recording
rec-bar-stop-a11y = Stop and save recording, { $time } elapsed

## Scroll capture HUD
scroll-title = Scroll Capture
scroll-title-size = Scroll Capture  ·  { $w }×{ $h }
scroll-auto = Auto Scroll
scroll-scrolling = Scrolling…
scroll-stop = Stop
scroll-cancel-help = Cancel (Esc)
scroll-status-accessibility = Allow Accessibility in Settings, then retry Auto Scroll.
scroll-status-no-target = No target app found. Cancel and reselect the area.
scroll-cancel-a11y = Cancel Scrolling Capture
scroll-pause-auto-a11y = Pause Automatic Scrolling
scroll-start-auto-a11y = Start Automatic Scrolling
scroll-stop-a11y = Stop Scrolling Capture
scroll-hud-a11y = Scroll Capture, { $w } by { $h } points
region-a11y = Select an area to capture. Drag to draw it, Return to capture, Escape to cancel.
