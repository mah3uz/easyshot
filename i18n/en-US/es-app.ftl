## App-level toasts, desktop notifications and modal alerts.
## Some platform wording still needs adapting to Linux.

-app-name = easyshot

## Toasts
toast-saved = Saved
toast-saved-message = Saved to { $path }
toast-recording-saved = Recording saved!
toast-recording-saved-message = Saved to { $path }
toast-video-exported = Video exported
toast-video-exported-message = Saved to { $path }
toast-recording-busy = A recording is already running
toast-recording-no-system-audio = Recording without system sound
toast-recording-no-input = Recording without clicks and keys
toast-recording-no-input-message = { -app-name } can’t read your keyboard and mouse. Add yourself to the input group with sudo usermod -aG input $USER, then log out and in.
toast-recording-input-pending = Recording without clicks and keys
toast-recording-input-pending-message = You were added to the input group, but this session started before that. Log out and in.
toast-recording-input-not-built = Recording without clicks and keys
toast-recording-input-not-built-message = This build of { -app-name } can’t record clicks and keys.
toast-recorder-missing = No screen recorder is installed
toast-recorder-missing-message = Install gpu-screen-recorder or wf-recorder (Arch: sudo pacman -S wf-recorder).
toast-recording-no-audio-device = Couldn’t start recording
toast-recording-no-audio-device-message = The microphone or sound device wasn’t found. Pick another in Settings › Recording, or turn the sound off.
toast-recording-area-off-monitor = Couldn’t start recording
toast-recording-area-off-monitor-message = The area you chose isn’t on any monitor. Choose it again.
toast-recording-busy-message = Stop it from the tray or the recording controls first.
toast-recording-save-failed = Couldn't save recording
toast-copy-failed = Copy Failed
toast-capture-save-failed = Couldn’t save capture
toast-capture-save-failed-message = The capture is still in the deck. Check the save folder in General settings and try Save again.
toast-capture-unavailable = Capture unavailable
toast-capture-unavailable-message = This file has been moved or deleted.
toast-scroll-capture-failed = Couldn’t capture scrolling area
toast-scroll-capture-failed-message = Check Screen & System Audio Recording permission, then select the area and try again.
toast-screenshot-failed = Couldn’t capture screenshot
toast-ocr-failed = Couldn’t recognize text
toast-agent-redact-missing = Agent screenshots are not hiding secrets
toast-agent-redact-missing-message = Hiding secrets needs Tesseract. Install it (Arch: sudo pacman -S tesseract tesseract-data-eng) or turn off redaction in Settings.
toast-ocr-missing = Text recognition needs Tesseract
toast-ocr-missing-message = Install it to read text (Arch: sudo pacman -S tesseract tesseract-data-eng). QR codes are read without it.
toast-ocr-empty = No text found
toast-ocr-empty-message = Try selecting a clearer text area.
toast-copied = Copied
toast-copied-color = { $hex } copied to clipboard
toast-copied-text = Text copied to clipboard

## Notifications
notice-open = Open
notice-show-in-folder = Show in folder
notice-edit = Edit

## Alerts and confirmation dialogs
alert-recording-inputs-title = Some recording inputs are unavailable
alert-input-access-title = { $device ->
        [camera] Camera access needed
        [microphone] Microphone access needed
       *[other] Input access needed
    }
alert-input-access-message = Allow { -app-name } to use the { $device ->
        [camera] camera
        [microphone] microphone
       *[other] input
    } in Privacy & Security, then select it again.
alert-input-restricted-message = { -app-name } can't use the { $device ->
        [camera] camera
        [microphone] microphone
       *[other] input
    } because access is restricted on this computer.

## Errors
toast-error-generic = Something went wrong
toast-unsupported = Not available here
toast-unsupported-message = Your system or compositor doesn’t support this.
toast-window-closed = Window closed
toast-window-closed-message = { $window } is closed, so there is nothing to capture again.
notify-config-invalid = Settings file has an error
notify-config-invalid-body = Line { $line }, column { $col }: { $message } Your settings are unchanged until it is fixed.
toast-save-failed = Save Failed
toast-recovered-edits = Recovered unsaved edits
toast-settings-invalid = Settings file has an error
toast-settings-invalid-message = Line { $line }, column { $col }: { $message } Your settings are unchanged until it is fixed.
toast-settings-save-failed = Couldn’t save settings
toast-settings-save-failed-message = { $error }
picker-folder-title = Choose a folder
