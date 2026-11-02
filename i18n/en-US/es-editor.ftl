## Image editor strings. Shortcuts are passed in as { $shortcut }.
## Shared button words live in es-ui.ftl (ui-*); the transfer toast lives there too (transfer-*).

## Window / title bar
editor-window-title-default = Image Editor
editor-undo = Undo
editor-redo = Redo
editor-save-flash = Saved
editor-save-help = Save screenshot to your save folder ({ $shortcut })
editor-export = Export
editor-export-a11y = Export image
editor-exporting-a11y = Exporting image
editor-export-help = Save the finished image to your computer
editor-save = Save
editor-save-help-plain = Save screenshot to your save folder
editor-saving-a11y = Saving screenshot
editor-history-a11y = Undo and redo
editor-crop-aspect-help = Aspect ratio
editor-crop-reset-help = Reset the selection to the whole image
editor-crop-apply = Crop

## Tool row
editor-tool-select = Select
editor-tool-rectangle = Rectangle
editor-tool-solid-rectangle = Solid rectangle
editor-tool-circle = Circle
editor-tool-line = Straight line
editor-tool-arrow = Arrow
editor-tool-freehand = Freehand
editor-tool-numbered-circle = Numbered circle
editor-tool-text = Text
editor-tool-highlight = Highlight
editor-tool-highlight-help = Draw an area to keep visible; everything outside is dimmed
editor-tool-pixelate = Pixelate
editor-tool-blur = Blur
editor-tool-row-a11y = Tools
editor-shortcut-help = { $title } ({ $shortcut })
editor-aspect-help = Canvas aspect ratio
editor-rotate-menu = Rotate and flip
editor-style-bar-a11y = Annotation style
editor-stroke-width = Stroke width
editor-aspect-auto = Auto
editor-aspect-1-1 = 1:1
editor-aspect-4-3 = 4:3
editor-aspect-3-2 = 3:2
editor-aspect-16-9 = 16:9
editor-crop = Crop
editor-crop-help = Crop Image — click again to cancel
editor-rotate-help = Rotate left 90°. Open the menu for Rotate Right and Flip.
editor-rotate-help-shortcut = Rotate left 90°. Open the menu for Rotate Right and Flip. ({ $shortcut })
editor-rotate-left = Rotate Left
editor-rotate-right = Rotate Right
editor-flip-horizontal = Flip Horizontal
editor-flip-vertical = Flip Vertical
editor-redact = Smart Redaction
editor-redact-body = Automatically hide sensitive information
editor-redact-pixelate = Pixelate
editor-redact-blur = Blur
editor-redact-scanning = Scanning screenshot…
editor-redact-none-found = No sensitive text found.
editor-redact-added =
    { $count ->
        [one] Added { $count } redaction
       *[other] Added { $count } redactions
    }
editor-color = Color
editor-color-picker-a11y = Annotation color
editor-stroke-help = { $count } px
editor-stroke-a11y =
    { $count ->
        [one] { $count } pixel
       *[other] { $count } pixels
    }
editor-text-style = Text Style
editor-redact-strength = Strength
editor-delete-help = Delete selected annotation ({ $shortcut })
editor-delete-a11y = Delete Selection
editor-swatch-black = Black
editor-swatch-red = Red
editor-swatch-orange = Orange
editor-swatch-yellow = Yellow
editor-swatch-green = Green
editor-swatch-turquoise = Turquoise
editor-swatch-blue = Blue
editor-swatch-purple = Purple
editor-swatch-pink = Pink
editor-swatch-white = White
editor-swatch-custom-color = Custom color
editor-swatch-custom = Custom
editor-inspector-hide = Hide Inspector
editor-inspector-show = Show Inspector
editor-inspector-toggle-a11y = Toggle image inspector
editor-tip-title = Point out what matters
editor-tip-message = Choose Arrow, then drag on the image. Click the tool again to return to Select. Undo with { $shortcut }.

## Text style popover
editor-font-family-help = Font family
editor-font-pro = Inter
editor-font-compact = Inter Tight
editor-font-rounded = Nunito
editor-font-size-decrease = Smaller text
editor-font-size-increase = Larger text
editor-font-size-a11y = Font size { $size }
editor-text-bold = Bold
editor-text-bold-glyph = B
editor-text-italic = Italic
editor-text-italic-glyph = I
editor-text-underline = Underline
editor-text-underline-glyph = U
editor-text-align = Alignment
editor-text-align-start = Align left
editor-text-align-center = Align center
editor-text-align-end = Align right

## Inspector: preset bar
editor-preset-menu-title = Presets…
editor-preset-help-applied = Choose a preset. { $name } is applied to new screenshots.
editor-preset-help = Choose a background, layout, camera, blur, border, and watermark preset
editor-preset-a11y = Background preset
editor-preset-a11y-value = { $title }
editor-preset-a11y-modified = , modified
editor-preset-a11y-wallpaper-missing = , wallpaper missing
editor-preset-add-help = Save current settings as a preset
editor-preset-add-a11y = Add Preset
editor-preset-delete = Delete { $name }
editor-preset-menu-saved = Saved Presets
editor-preset-menu-current = Current Settings
editor-preset-menu-wallpaper-missing = Wallpaper Missing
editor-preset-menu-delete-item = Delete “{ $name }”…
editor-preset-menu-default = Default Preset
editor-preset-menu-none = None
editor-preset-menu-empty = No Saved Presets
editor-preset-new-title = New Preset
editor-preset-new-body = Save the current background, layout, camera, blur, border, and watermark settings. It will be used for new screenshots.
editor-preset-new-placeholder = Preset name
editor-preset-error-wallpaper = The selected wallpaper can’t be found. Choose it again before saving.
editor-preset-error-duplicate = A preset with this name already exists.
editor-preset-delete-title = Delete Preset?
editor-preset-delete-message = “{ $name }” will be removed from your saved presets. The current canvas settings won’t change.

## Inspector: sections
editor-section-background = Background
editor-section-camera = Camera
editor-section-blur = Progressive Blur
editor-section-border = Border
editor-section-watermark = Watermark
editor-section-expanded = Expanded
editor-section-collapsed = Collapsed
editor-section-collapse-hint = Collapse section
editor-section-expand-hint = Expand section
editor-clear-background = Remove background
editor-clear-camera = Reset camera
editor-clear-blur = Reset progressive blur
editor-clear-border = Reset border
editor-enable-blur-a11y = Enable progressive blur
editor-enable-border-a11y = Enable border

## Inspector: background
editor-bg-none = No Background
editor-bg-fill-library = Fill library
editor-bg-fill-color = Color
editor-bg-fill-gradient = Gradient
editor-bg-fill-wallpaper = Wallpaper
editor-bg-layout = Layout
editor-bg-padding = Padding
editor-bg-shadow = Shadow
editor-bg-corners = Corners
editor-bg-shadow-style = Shadow style
editor-bg-shadow-soft = Soft
editor-bg-shadow-long = Long
editor-bg-shadow-glow = Glow
editor-bg-shadow-crisp = Crisp
editor-bg-alignment = Alignment
editor-bg-align-top-left = Top left
editor-bg-align-top = Top
editor-bg-align-top-right = Top right
editor-bg-align-left = Left
editor-bg-align-center = Center
editor-bg-align-right = Right
editor-bg-align-bottom-left = Bottom left
editor-bg-align-bottom = Bottom
editor-bg-align-bottom-right = Bottom right
editor-bg-align-a11y = { $title } alignment
editor-bg-align-pad-help = Image alignment
editor-bg-align-pad-disabled-help = Reset Camera to use alignment
editor-bg-align-pad-a11y = Image alignment
editor-bg-color-black = Black
editor-bg-color-white = White
editor-bg-color-graphite = Graphite
editor-bg-color-red = Red
editor-bg-color-orange = Orange
editor-bg-color-yellow = Yellow
editor-bg-color-green = Green
editor-bg-color-blue = Blue
editor-bg-color-purple = Purple
editor-bg-color-blush = Blush
editor-bg-color-mint = Mint
editor-bg-color-sky = Sky
editor-bg-color-lavender = Lavender
editor-bg-color-peach = Peach
editor-bg-color-sage = Sage
editor-bg-color-sand = Sand
editor-bg-custom = Custom
editor-bg-custom-help = Choose any background color
editor-bg-gradient-blush-lavender = Blush / Lavender
editor-bg-gradient-peach-apricot = Peach / Apricot
editor-bg-gradient-mint = Mint
editor-bg-gradient-powder-blue = Powder Blue
editor-bg-gradient-butter-yellow = Butter Yellow
editor-bg-gradient-lilac-periwinkle = Lilac / Periwinkle
editor-bg-gradient-sage-green = Sage Green
editor-bg-gradient-coral-rose = Coral / Rose
editor-bg-gradient-aqua-cyan = Aqua / Cyan
editor-bg-gradient-dusty-mauve = Dusty Mauve
editor-bg-wallpaper-recent = Recent
editor-bg-wallpaper-system = macOS
editor-bg-wallpaper-add-help = Choose wallpaper
editor-bg-wallpaper-panel-title = Choose Background Wallpaper

## Inspector: camera
editor-camera-angle = Camera angle
editor-camera-angle-help = Orbit the camera around the card center, then roll the view
editor-camera-tilt-x = Tilt X
editor-camera-tilt-y = Tilt Y
editor-camera-roll = Roll
editor-camera-framing = Framing
editor-camera-framing-help = Use a lower FOV for a calmer lens, then frame with Zoom and Pan
editor-camera-fov = FOV
editor-camera-zoom = Zoom
editor-camera-pan-x = Pan X
editor-camera-pan-y = Pan Y
editor-camera-card-rotation = Card rotation
editor-camera-card-rotation-help = Rotate the card around its own horizontal and vertical center axes
editor-camera-rotate-x = Rotate X
editor-camera-rotate-y = Rotate Y

## Inspector: progressive blur
editor-blur-apply-to = Apply to
editor-blur-target-screenshot = Screenshot
editor-blur-target-screenshot-help = Blur only the screenshot content inside its frame
editor-blur-target-scene = Scene
editor-blur-target-scene-help = Blur the transformed screenshot together with its background
editor-blur-mode = Mode
editor-blur-mode-radial = Radial
editor-blur-mode-directional = Directional
editor-blur-strength = Strength
editor-blur-falloff = Falloff
editor-blur-focus-size = Focus Size
editor-blur-focus-size-clipped-help = Choose the size of the sharp area inside the frame
editor-blur-focus-size-bleed-help = Choose the size of the sharp area across the scene
editor-blur-direction = Direction
editor-blur-focus-position = Focus position
editor-blur-focus-pad-help = Drag to move the sharp focal area. Double-click to center.
editor-blur-focus-pad-a11y = Blur focus position
editor-blur-focus-pad-value = X { $x }, Y { $y }, size { $size } percent

## Inspector: border and watermark
editor-border-color = Color
editor-border-thickness = Thickness
editor-border-opacity = Opacity
editor-watermark-add = Add watermark
editor-watermark-edit-help = Edit watermark text
editor-watermark-placeholder = Watermark text
editor-watermark-density = Density
editor-watermark-size = Size
editor-watermark-angle = Angle
editor-watermark-opacity = Opacity
editor-watermark-color = Color

## Inspector: slider value field
editor-slider-value-help = Enter an exact value for { $title }
editor-slider-value-a11y = { $title } value
editor-slider-track-hint = Drag horizontally to adjust, or edit the value field

## Footer / canvas / states
editor-zoom-a11y = Canvas zoom, { $percent } percent
editor-zoom-in = Zoom In
editor-zoom-out = Zoom Out
editor-zoom-fit = Fit Canvas
editor-crop-size-help = Crop size
editor-lowres-help = Why is this preview low resolution?
editor-lowres-text = Low-res preview to save memory - exports stay full quality
editor-share-help = Upload and copy a share link
editor-copy-help = Copy the finished image to the clipboard ({ $shortcut })
editor-error-load = Unable to load screenshot.
editor-copy = Copy
editor-copied = Copied
editor-copying-a11y = Copying image
editor-copy-help-plain = Copy the finished image to the clipboard
editor-close = Close
editor-loading-a11y = Loading screenshot
editor-inspector-a11y = Inspector
editor-error-transform = Unable to transform the image. Your image is unchanged. Try again.
editor-error-undo-preserve = Unable to preserve the image for undo. Try again.
editor-error-redo-preserve = Unable to preserve the image for redo. Try again.
editor-error-crop = Unable to crop the image.
editor-error-copy = Failed to copy annotation: { $error }
editor-error-export = Failed to export image: { $error }
editor-error-save = Failed to save annotation: { $error }
editor-save-panel-title = Export Screenshot
editor-close-guard-message = Do you want to save the changes to “{ $name }”?
editor-close-guard-this-screenshot = this screenshot
editor-close-guard-info = Your changes since the last save will be lost if you don't save them.
editor-close-guard-save = Save and Close
editor-close-guard-discard = Discard Changes
