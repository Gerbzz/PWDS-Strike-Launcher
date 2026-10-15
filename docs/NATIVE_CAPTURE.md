# Native screenshot capture (draft branch)

The `pwds-strike-launcher-ui` binary accepts `--capture catalog NEW.png` or
`--capture library NEW.png`. It renders the real egui window, waits for two
frames, requests an egui native screenshot, writes a PNG at a new path, and
closes. A missing screenshot, timeout, encoding error, or existing destination
returns a nonzero exit code. Running without arguments opens the ordinary UI.

The [native preview workflow](../.github/workflows/native-preview.yml) builds
the pushed branch on a standard Windows GitHub-hosted runner. It captures both
pages from the bundled empty release manifest and the runner's fresh local
installed state, checks PNG signatures and size, and retains the images and
source commit in a short-lived artifact. It does not publish either image to
the README automatically.

Before publication, download that artifact, inspect both actual images, check
the run's source SHA and labels, and record the final files' hashes. The
catalog should accurately show unavailable install controls; the library
should accurately show no launcher-managed apps installed. These are empty
states, not evidence that release installation or app launching works.
