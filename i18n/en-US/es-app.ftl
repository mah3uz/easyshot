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
toast-recording-busy-message = Stop it from the tray or the recording controls first.
toast-recording-save-failed = Couldn't save recording
toast-copy-failed = Copy Failed
toast-capture-save-failed = Couldn’t save capture
toast-capture-save-failed-message = The capture is still in the deck. Check the save folder in General settings and try Save again.
toast-screenshots-saved =
    { $count ->
        [one] Screenshot saved!
       *[other] { $count } screenshots saved!
    }
toast-capture-unavailable = Capture unavailable
toast-capture-unavailable-message = This file has been moved or deleted.
toast-no-previous-region = No previous region
toast-no-previous-region-message = Select an area with { -app-name }’s adjustable selector first.
toast-clipboard-open-failed = Couldn’t open clipboard image
toast-clipboard-open-failed-message = The image is still on the clipboard. Check disk space and try again.
toast-no-capture = No capture available
toast-no-capture-edit-clipboard-message = Copy an image or take a screenshot first.
toast-no-capture-message = Take a screenshot or open Media Gallery to find a saved capture.
toast-prepare-failed = Couldn’t prepare capture
toast-prepare-failed-message = The screenshot is still available. Check disk space and try again.
toast-prepare-failed-original-message = The original screenshot is still available in the preview.
toast-scroll-finish-failed = Couldn’t finish scrolling capture
toast-scroll-finish-failed-message = The image could not be prepared. Select the area and try again. { $error }
toast-scroll-capture-failed = Couldn’t capture scrolling area
toast-scroll-capture-failed-message = Check Screen & System Audio Recording permission, then select the area and try again.
toast-screenshot-failed = Couldn’t capture screenshot
toast-pick-color-failed = Couldn’t pick color
toast-pick-color-rgb-message = This color couldn’t be converted to RGB. Try picking another pixel.
toast-ocr-working = Recognizing text…
toast-ocr-working-message = This can take a moment.
toast-ocr-failed = Couldn’t recognize text
toast-ocr-empty = No text found
toast-ocr-empty-message = Try selecting a clearer text area.
toast-copied = Copied
toast-copied-color = { $hex } copied to clipboard
toast-copied-text = Text copied to clipboard
toast-copy-retry = Couldn’t copy
toast-copy-retry-message = Try Copy again.
toast-update-available = Update Available
toast-update-available-message = Version { $version } is available

## Notifications
notice-open = Open
notice-show-in-folder = Show in folder
notice-edit = Edit
notify-export-complete = Export Complete
notify-export-complete-body = { $file } has been saved.

## Alerts and confirmation dialogs
alert-recording-restart-title = Start a new recording?
alert-recording-restart-message = This recording will be discarded and recording will start again.
alert-recording-restart-confirm = Start Over
alert-recording-discard-title = Discard this recording?
alert-recording-discard-message = This recording will be deleted without saving.
alert-recording-discard-confirm = Discard
alert-deck-close-title = Close all captures in the deck?
alert-deck-close-message = Unsaved captures in the deck will be discarded. Saved files are kept.
alert-deck-close-confirm = Discard Unsaved Captures
alert-restart-capture-title = Restart { -app-name } to capture
alert-restart-capture-message = macOS hasn’t applied { -app-name }’s Screen Recording permission yet. This can happen when { -app-name } opens at login. Restart { -app-name }, then try again. If it still can’t capture, check Privacy & Security › Screen & System Audio Recording.
alert-recording-inputs-title = Some recording inputs are unavailable
alert-recording-ended-early-title = The recording ended early, but your footage was saved
alert-recording-not-saved-title = The recording could not be saved
alert-recording-permission-title = Screen Recording permission needed
alert-recording-permission-message = { -app-name } can't record until it's allowed under Privacy & Security > Screen & System Audio Recording. After turning it on, quit and reopen { -app-name } - macOS applies the permission on relaunch.
alert-recording-permission-open = Open System Settings
alert-recording-start-failed-title = Couldn't start the recording
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
alert-quit-recording-title = A screen recording is still in progress
alert-quit-recording-message = { -app-name } will finish and save the recording before quitting. This can take a moment for a long recording.
alert-quit-recording-confirm = Finish Recording and Quit
alert-restart-required-title = Restart Required
alert-restart-required-message = { -app-name } needs to restart to activate keyboard shortcut overrides. Restart now?
alert-recovered-title =
    { $count ->
        [one] Recovered an interrupted recording
       *[other] Recovered { $count } interrupted recordings
    }
alert-recovered-message = The playable footage was preserved in History and can be reopened in { -app-name } Studio.
alert-camera-video-failed-title = The camera video could not be added
alert-camera-video-failed-message = Your screen, camera, and audio masters are safe in the recording project, but { -app-name } could not create the combined video: { $error }
alert-failure-message = { $detail } { $error }

## Errors
toast-error-generic = Something went wrong
toast-unsupported = Not available here
toast-unsupported-message = Your system or compositor doesn’t support this.
toast-window-closed = Window closed
toast-window-closed-message = { $window } is closed, so there is nothing to capture again.
notify-config-invalid = Settings file has an error
notify-config-invalid-body = Line { $line }, column { $col }: { $message } Your settings are unchanged until it is fixed.
notify-settings-save-failed = Couldn’t save settings
toast-save-failed = Save Failed
toast-recovered-edits = Recovered unsaved edits
toast-settings-invalid = Settings file has an error
toast-settings-invalid-message = Line { $line }, column { $col }: { $message } Your settings are unchanged until it is fixed.
toast-settings-save-failed = Couldn’t save settings
toast-settings-save-failed-message = { $error }
picker-folder-title = Choose a folder
