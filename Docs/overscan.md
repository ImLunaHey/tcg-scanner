# Overscan diagnostic

The fi-8170 macOS driver exposes `cropping=4`, labelled Overscan. Its guide says
this adds background areas above, below, left and right of the image:
https://origin.pfultd.com/downloads/IMAGE/macos/240/P2ZZ-0370-05ENZ0.pdf (page 17).
The installed feature enumeration does not expose a separate millimetre margin
or a backing-colour choice for Overscan. Values 0/1 select uncropped black/white
instead; they cannot be combined with value 4.

Local JSON-RPC scan requests accept optional `overscan:true`. This selects and
verifies cropping value 4, records `background:driverDetermined` in applied
settings, and restores the previous driver value after capture. The request must
omit `background` and disable `prepareCards`: backing colour and output geometry
must first be physically validated. Manual card preparation also rejects this
experimental mode rather than silently choosing the wrong boundary detector.

Existing requests and physical-button settings are unchanged. No brightness,
contrast, pre-pick, multifeed, paper-protection or feed-mode settings are changed.
This diagnostic does not promise the Windows screenshot's 4 mm margin.

Initial test: three user-confirmed expendable cards, 300 DPI, colour, duplex PNG.
Expected result: six originals. Inspect their background, leading/trailing edges,
dimensions and margins before deciding whether this mode improves edge capture.
