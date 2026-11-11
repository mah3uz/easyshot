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
toast-exported-without-chooser = Exported to your save folder
toast-exported-without-chooser-message = No file chooser is running, so it went to { $path }
toast-recording-busy = A recording is already running
toast-capture-busy = A capture is in progress
toast-recording-no-system-audio = Recording without system sound
toast-recording-mic-lost = Microphone disconnected
toast-recording-window-closed = The window closed
toast-recording-window-closed-message = The recording stopped. It’s saved.
toast-recording-window-lost = The window can’t be recorded any more
toast-recording-window-lost-message = The recording stopped. It’s saved.
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
toast-recording-window-unreachable = Couldn’t start recording
toast-recording-window-unreachable-message = { -app-name } couldn’t switch to that window’s workspace. Switch to it yourself, then choose it again.
toast-recording-busy-message = Stop it from the tray or the recording controls first.
toast-recording-save-failed = Couldn't save recording
toast-copy-failed = Copy Failed
toast-capture-save-failed = Couldn’t save capture
toast-capture-save-failed-message = The capture is still in the deck. Check the save folder in General settings and try Save again.
toast-capture-unavailable = Capture unavailable
toast-capture-unavailable-message = This file has been moved or deleted.
toast-open-unsupported = Can’t open that file
toast-open-unsupported-message = Easyshot opens PNG and JPEG images and MP4, WebM, MKV, MOV and GIF videos.
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
toast-undo = Undo
toast-deleted = Moved to the Trash
toast-discarded = Capture deleted
toast-delete-failed = Couldn’t delete the capture
toast-undo-failed = Couldn’t restore the capture
toast-undo-failed-message = It is still in the Trash.
toast-preview-failed = Can’t preview that capture

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
toast-settings-unknown-one = Unknown setting in config.toml
toast-settings-unknown-one-message = { $keys } is not a setting easyshot knows. It is kept in the file and ignored.
toast-settings-unknown-many = Unknown settings in config.toml
toast-settings-unknown-many-message = { $keys } are not settings easyshot knows. They are kept in the file and ignored.
toast-settings-unknown-key = { $key } (line { $line })
toast-settings-unknown-more = and { $more } more
toast-settings-save-failed = Couldn’t save settings
toast-settings-save-failed-message = { $error }
picker-folder-title = Choose a folder
picker-open-title = Open an image or video
picker-open-filter = Images and videos

## Sharing
share-failed-not-configured = Set up your bucket in Settings before sharing to it.
share-failed-keyring = { -app-name } couldn’t read your bucket keys from the keyring.
share-failed-network = { -app-name } couldn’t reach the share service. Check your connection and try again.
share-failed-file = { -app-name } couldn’t read the file to share.
share-failed-unexpected = Sharing didn’t work. Try again.
share-failed-cancelled = Sharing was cancelled.
share-failed-bad-request = The share service couldn’t accept this file.
share-failed-unauthorized = The share service didn’t accept this copy of { -app-name }. Try again.
share-failed-token-expired = The upload took too long. Try again.
share-failed-challenge-failed = The share service couldn’t check this copy of { -app-name }. Try again.
share-failed-origin-refused = The share service refused the request.
share-failed-not-found = This share is no longer on the share service.
share-failed-conflict = The share changed while it was uploading. Try again.
share-failed-too-large = This file is larger than the share service accepts.
share-failed-unsupported-type = The share service doesn’t accept this kind of file.
share-failed-version-unsupported = This version of { -app-name } can no longer share. Update { -app-name } to share again.
share-failed-rate-limited = Too many shares in a short time.
share-failed-quota-daily = You have shared as much as the service allows today.
share-failed-full = The share service is full right now.
share-failed-bucket-credentials = Your bucket refused your keys. Check them in Settings.
share-failed-bucket-bucket = Your bucket wasn’t found or refused the upload. Check its name and region in Settings.
share-failed-bucket-public-url = Your bucket’s public URL doesn’t serve the shared files. Check it in Settings.
share-failed-bucket-network = { -app-name } couldn’t reach your bucket. Check the endpoint and your connection.
share-failed-too-long = This clip is too long to share. { $minutes ->
        [0] Less than a minute of it fits.
        [one] A minute of it fits.
       *[other] { $minutes } minutes of it fit.
    } Trim it and try again.
share-retry-later = Try again later.
share-retry-minutes = Try again in { $minutes ->
        [one] a minute
       *[other] { $minutes } minutes
    }.
share-retry-hours = Try again in { $hours ->
        [one] an hour
       *[other] { $hours } hours
    }.

share-toast-preparing = Preparing…
share-toast-link-copied = Link copied
share-toast-link-copied-message = Uploading { $percent } %
share-toast-shared = Shared
share-toast-shared-message = Expires in { $days ->
    [one] 1 day
   *[other] { $days } days
}
share-toast-shared-bucket = Shared to { $bucket }
share-toast-failed = Couldn’t share
share-toast-cancel = Cancel
share-toast-open = Open
share-toast-copy-link = Copy Link
share-toast-retry = Retry
share-popover-a11y = Share
