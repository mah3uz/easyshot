# easyshot features

This file lists everything easyshot does, one line each, with the details in [MANUAL.md](MANUAL.md). A change a user would notice updates this file, MANUAL.md, the CHANGELOG and the website (`docs/site/`) in the same commit.

## What it needs

- Works on Hyprland and sway. Built for niri, but not tested on it. Basic capture on other Wayland compositors.
- Reads text from the screen when Tesseract is installed, and reads QR codes without it.
- Records the screen with gpu-screen-recorder or wf-recorder, and tells you how to install one if neither is there.
- Reports what your desktop and tools support with `easyshot doctor`.

More in the manual: [What it needs](MANUAL.md#what-it-needs).

## Installing and running

- Installs on Arch Linux from two AUR packages: one builds from source, one is prebuilt. On other distributions, one command installs the signed prebuilt release, for all users or just you.
- Starts by itself with each graphical session once the package is installed, and an update restarts it.
- Runs as one background program that every `easyshot` command talks to, and starts it if it is not running.
- Adds Easyshot to your application menu, where it opens the Home window.
- Opens `easyshot://` links.
- Lets you stop it starting for your user only.
- Sets no global keys until you choose them, so nothing clashes with your own.
- Suggests Print, Shift+Print, Alt+Print and Ctrl+Print at first run, where your compositor leaves them free.
- Writes your chosen keys to its own file for Hyprland, sway or niri and never edits your compositor config.
- Shows the one line to add to your compositor config, with a Copy button and a check that says Included or Not found.
- Reloads sway for you with a Reload button; Hyprland and niri reload on their own.
- Uses the desktop's global-shortcuts portal where it cannot write a keys file.

More in the manual: [Installing it](MANUAL.md#installing-it) and [Running it](MANUAL.md#running-it).

## Capturing

- Captures a region: the screen freezes, you drag a selection and adjust it, and the last area is preselected.
- Press Space while selecting to switch between drawing a region and picking a window.
- Runs one capture at a time: while you pick, another capture key switches the picker to that mode on the same frozen screen, and the same key closes it.
- Says "A capture is in progress" if you start a capture while one is already under way.
- Lets you take screenshots while a recording runs.
- Captures the previous target again: the same area, display or window.
- Tells you when the previous window has closed, instead of capturing.
- Captures the whole screen under the pointer.
- Captures a window by hovering over it and clicking.
- Counts down before a capture: pick first, then easyshot takes a fresh frame, so menus you open during the countdown are in the shot.
- Sets the countdown with `--timed` (3 seconds by default) or `--timed N` for 1 to 60 seconds.
- Sets the countdown from the capture bar's timer button: Off, 3, 5 or 10 seconds.
- Shows the countdown as a dial that springs in, each second rolling down into place.
- Reads text from a region, with or without line breaks.
- Reads QR codes from a region.
- Picks a colour from the screen and copies it as `#RRGGBB`.
- Captures a scrolling page as one tall image, up to 30000 pixels.
- Keeps a page's fixed top and bottom bars once, at the top and the end of the tall image.
- Keeps a strip at the side that does not scroll, such as an editor's minimap or a side panel, as the first view showed it, never repeated down the image; page content that reaches into it is kept whole. Cuts a scrollbar at the right edge off the image.
- Joins pages with a playing video or animation, whether it scrolls with the page or stays in place.
- Scrolls by hand with the wheel, a touchpad or the keyboard, wherever the pointer is.
- Auto Scroll parks the pointer on the capture bar between steps, so it and hover effects stay out of the image.
- Auto Scroll goes back half a step and tries again when a step does not join.
- Scroll capture starts by drawing a region; press Space to pick a window instead.
- Shift-click the capture bar's Scroll button to start scroll capture on the window picker.
- Reads a scrolled window from its own pixels where the compositor gives them, so windows on top of it stay out.
- Shows the size so far on a bar under the scrolling area, with a Stop button.
- Scrolls the page for you with Auto Scroll, where your compositor allows it, and stops at the end.
- Shows a preview of the tall image beside the area as it grows.
- Stops a scrolling capture when you press its shortcut again or click the ✕; Esc does not cancel it.
- Offers region captures that also copy, save, annotate or pin.
- Always freezes the screen during a capture.

More in the manual: [Capturing](MANUAL.md#capturing).

## After a capture

- Plays a shutter sound if you turn it on (in builds that include sound).
- Saves the file, copies it and opens the editor, as your settings say.
- Shows each capture as a card in a floating preview deck of up to 5 cards.
- Stacks the cards in the corner, newest in front with a count, and fans them out while you hover; a new card springs in, growing and rising into place as it fades in.
- Offers hover actions on a card, in slots you can set up.
- Lets you choose which actions a card shows in Settings › Overlay › Card Actions; Cloud Share is there, and opens the share popover.
- Shares a capture straight from its card: a popover asks for a title and 1, 3 or 7 days, a link is copied as the upload starts, and a toast shows progress, Cancel, then Open and Copy Link.
- Shares an image from the editor as you edited it, from Share in its title bar or the Share Image key.
- Shares a recording from the trim viewer, from Share in its title bar or the Share Video key, as trimmed and with your export choices.
- Marks a card whose capture has a live link with a small cloud.
- Keeps easyshot Free shares for the 1, 3 or 7 days you pick (images up to 10 MB, GIFs up to 20 MB, videos up to 200 MB, a longer video re-encoded to fit), then deletes them; visitors to an expired or deleted link see only "Page not found".
- Sends shares to your own S3-compatible bucket once its endpoint, bucket and public URL are filled in, with no expiry of its own; otherwise to easyshot Free, with no destination to choose and no first-share step.
- Opens a share link on a page with the image or video, Download and Copy link; links are unlisted, not private.
- Opens a card's Preview in a plain window with the image fitted; click or Z toggles 100%, Space or Esc closes it.
- On Hyprland, opens a card's Preview as a floating window in the middle of the screen, through a rule in the binds file.
- Moves a card's capture to the Trash with Delete, and a toast's Undo puts it back and returns the card.
- Turns Save into Show in Folder on a card that was already saved automatically.
- Lets you drag a card into another app or a file manager as a copy.
- Copies a recording from its card to the clipboard as an MP4 file for pasting into a file manager or chat app.
- Removes cards on their own, each on its own clock, pausing while you hover over the deck.
- Saves or clears every card at once with Save All and Clear All.
- Keeps cards through a restart for 24 hours.
- Shows a card in your default look and reopens the capture in the editor with that look still editable.
- Hides and shows the deck with `easyshot deck toggle`; the next capture shows it again.
- Drops a pin you never saved when you close it or restart.

More in the manual: [After a capture](MANUAL.md#after-a-capture) and [Sharing](MANUAL.md#sharing).

## Pinned screenshots

- Keeps a screenshot on top of other windows.
- Move a pin by dragging it.
- Resize a pin by scrolling; the top-left corner stays where it is.
- Close a pin with the button that fades in when you hover.
- Copy or close a pin from its menu.

More in the manual: [Pinned screenshots](MANUAL.md#pinned-screenshots).

## The tray menu

- Lists Region, Screen, Window, Record, Scrolling Capture, OCR and Pick Color, with your global shortcut beside each one you set.
- Shows Recent Captures: your eight newest screenshots and eight newest recordings, saved or not, and opens one with a click.
- Offers Media Gallery, Settings and Quit.
- Offers Unpin All Windows while a screenshot is pinned.
- Shows Stop Recording, Pause, Restart and Discard with the elapsed time while a recording runs, and a red dot on the icon.
- Shows a Revoke row and a badge on the icon while an agent has access.
- Starts a capture once your panel has closed the menu, so the menu is never in the shot.

More in the manual: [The tray menu](MANUAL.md#the-tray-menu).

## The editor

- Offers 13 tools, including shapes, lines, arrows, pens, text, a numbered counter, highlight, blur and pixelate.
- Select, move, resize and rotate shapes.
- Attach arrows to shapes so they follow.
- Edit text after you place it.
- Undo and redo.
- Crop, rotate and flip the image, and zoom and pan.
- Shows the title bar, the tool row, the image, an inspector on the right and a footer with zoom and Copy.
- Hide and show the inspector with the button at the right end of the tool row.
- Press Tab to move between the title bar, tool row, inspector and footer.
- Shows a tool's options in the inspector when you pick it, and opens the inspector if it was hidden.
- Go back to the image settings with the back arrow; click the tool you already have to bring its options back.
- Sets colour and stroke width for shapes and pens.
- Gives arrows a head for each end: none, arrow, triangle, square, dot, diamond, inverted or bar.
- Draws arrows and lines solid, dashed or dotted.
- Sets text font, colour, size, bold, italic, underline and alignment.
- Font list: the three bundled fonts first, then every installed font, searchable; a font that is not installed shows "(not installed)" and draws in Inter.
- Text in any script: a character the chosen font lacks draws in an installed font that has it, in text and in the watermark.
- Gives text an optional background and a border 2, 3 or 5 px wide.
- Sets the strength of blur and pixelate.
- Dims everything outside an area with the Highlight tool.
- Shows the options of a selected shape and a button to delete the selection.
- Applies a change to new shapes and to the selection as one undo step.
- Remembers your last colours, widths, arrowheads, text options and tool for next time.
- Resets a tool's options with Reset to defaults.
- Shows a pointer for each tool that says what a press makes: a cross with a rectangle, circle, line or arrow, a pen, a text cursor, a badge, a dotted square, or an eraser for Pixelate and Blur.
- Paints translucent colour over text without hiding it with the Highlighter (key M).
- Draws Highlighter strokes freehand or as a straight line, and press M again to switch.
- Snaps a straight Highlighter line to 15 degrees when you hold Shift.
- Starts new text at size 24; type any size from 4 to 300 in the Text options.
- Shows a dashed box where your text goes while you type; it is not saved or exported.
- Keeps annotations on the image: you cannot start, move, resize or rotate one past its edge.
- Crops by dragging corner and edge handles, or by dragging inside the selection to move it.
- Hold Alt while cropping to resize about the centre.
- Locks a crop to Freeform, Original, 1:1, 16:9, 9:16, 4:3 or 3:2.
- Shows the size the cropped image will have in the footer.
- Apply a crop with Enter or Crop; leave with Escape, Cancel or the crop button; Reset returns to the whole image. Reset, a new aspect and Cancel ease the crop frame into place.
- Rotates and flips from the menu beside the crop button.
- Moves annotations with the pixels under them when you crop, rotate or flip.
- Undoes crop, rotate and flip, keeping the last 16 image steps.
- Adds a background of a colour, one of 24 gradients or a wallpaper.
- Offers seven built-in wallpapers, or any image of your own, with the last 24 you chose kept in Recent.
- Makes your own gradients: two to four colours, any angle, a name; edit, rename or delete them, and edit a copy of a built-in one.
- Adjusts padding, corners, shadow, shadow style and alignment.
- Sets the shape of the whole picture: Free, 1:1, 4:3, 3:2 or 16:9, shown once there is a background.
- Tilts and rolls the picture in 3D, with lens, zoom, pan and card rotation.
- Keeps the picture sharp, as it will export, while you drag a camera or blur slider.
- Blurs the screenshot or the whole scene around a round or band-shaped sharp area, with Progressive Blur.
- Moves the sharp area by dragging its focus pad, and centres it with a double-click, gliding the knob home.
- Changes a slider by dragging anywhere on it or typing a value.
- Adds a border and a watermark.
- Applies presets of image settings.
- Hides private text with Smart Redaction (the eye-off button): emails, links, IP addresses, phone numbers, card numbers, tokens, API keys and passwords.
- Lets you choose Pixelate or Blur for Smart Redaction.
- Shows "Scanning screenshot…" while it works, flashes what it found and says what it hid.
- Undoes everything Smart Redaction added in one step with the message's Undo button.
- Holds the Smart Redaction message while the pointer is on it, like every toast.
- Keeps blur and pixelate strong enough that covered text cannot be read.
- Opens very large screenshots with a smaller preview to stay fast, while saving, exporting and copying use the full image.
- Shows every screenshot at half size if you turn on Always use a low-resolution preview.
- Saves with the edits kept, so you can reopen and keep editing.
- Exports PNG or JPEG.
- Exports to your save folder with a notice saying where when no file chooser is running, and uses the desktop's file chooser, light or dark and Reduce Motion as soon as its portal starts.
- Asks before closing with unsaved changes.
- Reopens your unsaved edits after an unexpected stop.

### Editor keys

- Arrow keys nudge the selection 1 pixel; hold Shift for 10.
- Esc steps back one level: it finishes the text you are typing, leaves crop, clears the selection, then switches to Select.
- Middle-button drag, or Space and drag on empty canvas, pans.
- Hold Space and drag an annotation to move it with any tool.
- Mouse wheel, Ctrl or Alt with the wheel, or pinch zooms about the pointer.
- Two-finger touchpad scroll pans.
- Ctrl+D duplicates the selection.
- Ctrl+C and Ctrl+V copy and paste shapes.
- Ctrl+Shift+C copies the image.
- Ctrl+] and Ctrl+[ move the selection one step forward or back.
- Ctrl+Shift+] and Ctrl+Shift+[ bring it to the front or send it to the back.
- Tab and Shift+Tab select the next or previous shape.

More in the manual: [The editor](MANUAL.md#the-editor) and [Editor keys](MANUAL.md#editor-keys).

## Home window and gallery

- Holds the gallery and Settings in one window, one tab each.
- Slides between the two tabs, and a Settings page rises into place as the last fades out.
- Opens from the application menu, from a bare `easyshot` or from an `easyshot://home` link.
- Opens on the gallery from `easyshot gallery` or the tray's Media Gallery.
- Opens on Settings from `easyshot settings`, the tray's Settings or an `easyshot://settings` link, and on a named page if you give one.
- Brings the one Home window to the front when you open it again.
- Has an Open… button that picks an image or video from anywhere on your computer: images open in the editor, videos in the trim viewer.
- Opens an image or video you drop onto the window.
- Offers Easyshot in your file manager's Open with for PNG, JPEG, GIF, MP4, WebM, MKV and MOV files, and `easyshot open FILE` does the same.
- Keeps a history of captures with search, sorting, icon and list views, and a trash.
- Sizes the gallery's previews with a toolbar slider, Ctrl+scroll, Ctrl+plus and Ctrl+minus (Ctrl+0 resets), and remembers the size.
- Shows each item's file extension after its name in the gallery, in a quieter colour.
- Opens a screenshot in the editor and a video in the trim viewer when you double-click it in the gallery.
- Keeps 100 history entries by default.
- Lists recordings in the gallery under Videos and All Media: unsaved ones, saved ones and trim exports, each with a first-frame thumbnail, a play badge and its length when known.
- Opens a video from the gallery or the tray in the trim viewer.
- Shows and trashes a video from the gallery as it does a screenshot.
- Marks shared items in the gallery with a cloud and when the link expires (or its bucket, or Expired), with Copy Link, Open Link, Copy File Link and Delete Cloud Share in the item's menu, and Remove for an expired link.
- Lists every link you shared under Cloud Shares in the gallery, newest first, even after its file is gone.
- Lists the tray's recent recordings from the same videos and updates them after a save, an export or a trash.

More in the manual: [The Home window](MANUAL.md#the-home-window) and [History and gallery](MANUAL.md#history-and-gallery).

## Recording

- Records an area, a display or a window, started from the capture bar and picker.
- Records a window's own pictures where the compositor allows it, so the video shows the window even when it is covered or on another workspace, keeps recording when the window moves to another workspace or monitor, and ends with the recording kept if the window closes or can no longer be captured; elsewhere it records the window's area on screen. [Window recordings](MANUAL.md#recording)
- The recording panel lists every open window grouped by workspace, each with its app's icon, title and app name; picking one on another workspace switches to it first. A long list scrolls and works from the keyboard.
- Records system sound unless you turn it off, and the microphone once you choose one.
- Records system sound and your microphone as two separate audio tracks, system sound first.
- Starts the capture bar's recording panel with your Recording settings: microphone, system sound and start delay.
- Says plainly when a recording can't start because a sound device or the chosen area is missing.
- Counts down, then shows a control bar with stop, pause, resume, restart and discard, springing in from the capture bar as it fades in.
- Drags the control bar by its grip anywhere on its screen, and remembers the spot for each screen.
- Mutes system sound and the microphone separately from the control bar during a recording, with a button for each sound being recorded.
- Highlights the area being recorded.
- Opens and closes the capture bar with `easyshot capture-bar`, its key or the tray.
- Closes the capture bar with Esc, or closes its open menu first.
- Leaves your keyboard free while the capture bar is open: click another app and type there.
- Records one thing at a time, and says "A recording is already running" if you start another.
- Shows Stop Recording in the tray while a recording runs.
- Keeps each recording as its own project folder.
- Shows a finished recording as a deck card and opens it in a trim viewer with export.
- Has Open in Studio in the trim viewer and in a video's gallery menu; it opens a panel saying Video Studio is coming soon.
- Saves recordings and exports to a dated file in `easyshot` inside your Videos folder, never replacing an earlier one.
- Records your clicks and shortcut keys, never plain typing, when Keystrokes is on, for click effects and key badges.
- Stops reading your keyboard and mouse while the screen is locked or switched away.
- Needs your user in the `input` group for Keystrokes, and tells you how when it is not.
- Shows whether Keystrokes is off, working or without access in Settings and in `easyshot doctor`.
- Exports MP4, WebM or GIF.
- Scales an export to 1080p, 720p or 480p without ever enlarging it.
- Exports in High, Balanced or Small quality.
- Gives an export a Natural, Vivid, Muted or Mono look, with a brightness slider, previewed in the player.
- Plays a recording's system sound and microphone tracks together in the trim viewer, each with its own volume slider (system sound starts at 50%), and mixes them into one track on export.
- Exports an MP4 without re-encoding when you keep its size and colour (Fast).
- Remembers your last export choices.
- Puts the control bar for a whole-screen recording on another monitor, or in the bottom-right corner (and in the video) if there is only one.
- Turns that control bar off in Settings; the tray's Stop Recording still works.
- Never takes your keyboard with the recording control bar, so you keep typing in the app you record.

More in the manual: [Recording](MANUAL.md#recording).

## Settings

- Has Home, General, Capture, Overlay, Recording, Shortcuts, Agents and About pages.
- Follows your desktop's light or dark choice with the System theme, or stays Light or Dark.
- Follows your desktop's animation setting as Reduce Motion: with it, every transition happens at once.
- Eases windows, pages, cards, tooltips and editor controls into place; tooltips open once the pointer rests on a control for 1.5 seconds, never jumping from one to the next, and hide by themselves after time to read them.
- Applies every change at once and saves it to `config.toml`.
- Applies edits you make to `config.toml` by hand while easyshot runs.
- Shows a toast naming keys in `config.toml` that easyshot does not know, such as a misspelt one.
- Keeps the last good settings and shows the line and column if the file has an error.
- Shows the capture buttons with their keys, your five latest captures and a Setup list on Home.
- Saves screenshots automatically unless you turn that off; Capture Region & Save still writes a file.
- Puts the selector on a screen you choose, or on the pointer's screen.
- Opens the capture bar when easyshot starts, if you turn that on.
- Opens editors in full screen, if you turn that on.
- Opens the trim viewer when a recording stops, if you turn that on.
- Lists every action on Shortcuts: click a key field and press a key to set it.
- Shows the command your compositor runs for each global shortcut.
- Checks which keys are already bound and which run something else.
- Explains options whose names say little in a tooltip (overlay layouts, codecs, containers, agent consent), and names each key field's clear button in one.
- Shows the version, the website, the source and the issue tracker on About, and Show Onboarding Again.
- Sets the editor's arrow-key steps and its zoom anchor in `config.toml`.
- Controls agent access, consent, scope and secret hiding on Agents.
- Says on Sharing where shares go (easyshot Free, or your bucket once it is filled in), sets how long free links last while Free is in use, keeps bucket keys in your keyring or a private file, and tests the bucket connection.

More in the manual: [Settings](MANUAL.md#settings).

## Command line

- Sends every `easyshot` command to the daemon and waits for it to finish.
- Opens the Home window with a bare `easyshot`, and lists commands with `easyshot --help`.
- Cancels a command with Ctrl+C.
- Starts the daemon when it is not running, or tells you how.
- Shares an image, a video or a recording from the command line with `easyshot share FILE` and prints the link.
- Opens a link, an image or a video with `easyshot open`: a PNG or JPEG in the editor, an MP4, WebM, MKV, MOV or GIF in the trim viewer.
- Exits 0 when done, 1 on error, 2 when cancelled and 3 when the daemon is not running.
- Prints the version, with the commit for a git build, from `easyshot --version`.
- Prints a completion script for bash, zsh and fish, which completes subcommands, flags, modes, settings pages, links, image and video files, and, for the agent commands' `--window`, the ids and titles of the open windows.
- Captures with `capture region`, `fullscreen`, `window`, `previous`, `timed` or `scroll`.
- Adds `--copy`, `--save`, `--annotate` or `--pin` to `capture region`.
- Adds `--timed [N]` to region, window and fullscreen captures.
- Adds `--window` to `capture scroll` to start on the window picker instead of a region.
- Reads text with `ocr`, and `ocr --single-line` for one line.
- Picks a colour with `color-picker`.
- Controls recording with `record` and its `area`, `stop`, `pause`, `restart` and `discard` actions.
- Toggles the capture bar with `capture-bar` and opens the gallery with `gallery`.
- Puts the latest capture back on the deck with `restore-last`, and pins it with `pin-last`.
- Removes every pin with `unpin-all`.
- Opens the image on the clipboard in the editor with `edit-clipboard`.
- Opens Settings on a page with `settings [page]`.
- Hides the deck, saves all cards or closes all cards with `deck toggle`, `save-all` and `close-all`.
- Quits the daemon with `quit` and runs it in the foreground with `daemon`.
- Summarises what works, the chosen providers and the tools to install with `doctor`, in colour on a terminal.
- Prints rejected providers, Wayland globals, portals and tool versions with `doctor --raw`.
- Prints that report as JSON with `doctor --json`.
- Writes a tar of the report and logs, without images, with `doctor --bundle`.
- Accepts nine `easyshot://` links, with no arguments: region, fullscreen, window and scroll capture, record, ocr, color-picker, home and settings.
- Makes a `record` link ask you to select the area first, so it never records the whole screen.

More in the manual: [Command line](MANUAL.md#command-line).

## Agent use

- Gives AI agents and scripts `easyshot agent` (also `easyshot ai`).
- Lists your outputs and your open windows.
- Takes a screenshot of an output, an area, a window or the focused output and prints the PNG path.
- Writes the screenshot to a path you name with `--out`.
- Waits 0 to 60 seconds first with `--delay`.
- Shows no deck card or notification with `--no-ui`.
- Waits for the result, or returns at once with `--no-wait`.
- Covers secrets such as keys, passwords, emails and card numbers in agent screenshots.
- Re-encodes a recording as MP4, WebM or GIF with size, quality, trim, look and brightness options.
- Refuses to replace an existing file unless you add `--force`.
- Draws arrows, boxes, ellipses, text, numbered badges, highlights, blur and pixelation on an image from a JSON file, with an optional background.
- Does the drawing without the daemon and without agent access, and checks the file strictly.
- Reads text and QR codes from a target or a PNG or JPEG file, in the languages you name.
- Starts, stops, checks and discards a recording, with audio, frame rate, keystrokes and a time limit (120 seconds by default, 600 at most).
- Stops an agent's recording, and keeps it, when its access ends.
- Prints exactly one JSON object with `--json`, errors included.
- Uses a separate exit code for each kind of error.
- Keeps agent access off until you turn it on, and asks you in a dialog first.
- Tells you in one plain sentence, in `error.hint`, what went wrong and what to do when an agent command fails.
- Shows, reads and changes access and settings with `agent enable`, `disable`, `status` and `config`.
- Answers only your own user.
- Asks for consent once per session by default, or every time, or never.
- Lets you allow a request for 1 hour, until you log out, or always, or deny it.
- Denies a request when you press Esc, close the dialog or leave it for 60 seconds.
- Shows a badge on the tray icon and a one-click Revoke while access is granted.
- Limits agents to the outputs and windows you name.
- Logs each agent request with its time, command, target, result and program.
- Installs a skill that teaches agents how to use easyshot, for Claude Code, Codex, Gemini CLI and Cursor.
- Installs the skill for your user or a project, and removes or prints it.
- Runs a Model Context Protocol server with `easyshot agent mcp`, with tools to list, capture, read text, record, export and annotate.
- Returns MCP screenshots scaled to 1280 pixels on the long edge, with secrets covered.

More in the manual: [Agent use](MANUAL.md#agent-use), [Annotating an image](MANUAL.md#annotating-an-image), [Hiding secrets](MANUAL.md#hiding-secrets), [Turning agent access on](MANUAL.md#turning-agent-access-on), [Consent](MANUAL.md#consent), [Scope](MANUAL.md#scope), [The log](MANUAL.md#the-log), [The skill for your agent](MANUAL.md#the-skill-for-your-agent) and [The MCP server for your agent](MANUAL.md#the-mcp-server-for-your-agent).

## Files and notifications

- Keeps settings, your last annotation style, screenshots, recordings, logs and agent records in standard folders.
- Saves screenshots to `Screenshots` in Pictures and recordings to `easyshot` in Videos.
- Opens a file made by a newer easyshot read-only and never changes it.
- Writes a crash report on your machine only, and offers to open it at the next start.
- Tells you where a saved file went, and opens it when you click the notification.
- Keeps a toast up for 2.5 seconds, an error toast for 5 and an Undo toast for 8, and holds it while the pointer is on it.
- Springs a toast in, growing and rising into place, and shrinks it away as it fades.
- Offers Show in folder on a notice, and Edit on a capture's notice.

More in the manual: [Files](MANUAL.md#files).

## When your desktop lacks something

- Crops a window out of the frozen screen when the compositor cannot capture windows.
- Shows a list of windows when the compositor gives no window positions.
- Leaves out Auto Scroll when the compositor cannot send input; manual scrolling still works.
- Saves an export to your save folder and tells you where when there is no file picker.
- Waits for your panel's tray at login, adds its icon when the panel appears and again if the panel restarts.
- Opens the capture bar at start only if no tray has appeared within 10 seconds.
- Greys out Record with an install hint when there is no recorder.
- Reads only QR codes when Tesseract is missing.
- Gives its windows app IDs for compositor rules: `dev.easyshot.app`, plus `.editor`, `.onboarding`, `.preview` and `.player`.

More in the manual: [When your compositor lacks something](MANUAL.md#when-your-compositor-lacks-something).

## Limits

- Lists what version 1 does not include in [Limits in version 1](MANUAL.md#limits-in-version-1).

## Troubleshooting

- Starts with `easyshot doctor`, and `EASYSHOT_LOG` changes what is logged.
- Explains why the pointer hides when you press Space in the editor, and how to turn that off in Hyprland or sway.

More in the manual: [Troubleshooting](MANUAL.md#troubleshooting).
