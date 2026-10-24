# Privacy

This application is an offline desktop program.

- **No telemetry, analytics or crash reporting.** Nothing about you or your use is collected or sent anywhere.
- **No network access.** The application makes no network requests and listens on no network port. (Development builds
  talk to a local dev server on `localhost` only.) It has no auto-updater and no accounts or sign-in.
- **Your files stay on your computer.** When you import a deck, the application copies it into its own data directory
  (in a `decks` folder) together with the images extracted from it. Scene media you add is kept in
  a `scene_media` folder there. Slide position and timer state are saved
  there so a restart can resume. Settings, such as the chosen presentation display, are stored in a
  TOML file in the application's config directory. Both directories are under your user profile and
  you can delete them at any time; uninstalling the application does not need them.
- **No secrets are stored.**

The Windows installer can include Microsoft's WebView2 runtime installer (offline mode), which the application uses to
render its windows. WebView2 is a Microsoft component with its own privacy terms.

If this changes in a future version (for example an opt-in update check), this document and the changelog will say so
before the release.

Questions: see [SECURITY.md](../SECURITY.md) for how to contact the maintainer.
