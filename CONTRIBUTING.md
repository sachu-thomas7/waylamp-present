# Contributing

Thanks for helping. This project is GPL-3.0-or-later; by contributing you agree that your
contribution is licensed under the same terms. Please read the
[Code of Conduct](./CODE_OF_CONDUCT.md) first.

## Proposing changes

- **Bugs**: open an issue using the bug template. Include the version, operating system, display
  setup and, for import problems, a sample file that contains no private data.
- **Features**: open an issue first and describe the problem you want solved. The scope is
  presentation control; live production (cameras, streaming) is developed on the
  `feature/live-production` branch. See [dev-docs/product_roadmap_2026.md](./dev-docs/product_roadmap_2026.md).
- **Small fixes** (typos, docs, obvious bugs) can go straight to a pull request.
- **Security issues**: do not use public issues; see [SECURITY.md](./SECURITY.md).

## Building and testing

See the README for prerequisites. Everything is checked by one script:

```bash
bash scripts/verify.sh quick   # while editing
bash scripts/verify.sh push    # before opening a PR (this is what CI runs)
bash scripts/verify.sh deps    # when you change dependencies
```

The script cannot launch the application. If you changed window wiring, IPC, capabilities or
anything in `src-tauri/src/app.rs`, also run `cargo tauri dev` and check both windows by hand.
Architecture rules that tests enforce are listed in [CLAUDE.md](./CLAUDE.md) ("Architecture invariants").

## Pull request checklist

- [ ] `bash scripts/verify.sh push` passes locally.
- [ ] New behaviour has a test; a bug fix has a test that fails without the fix.
- [ ] New IPC commands are added to the handler list and to `src-tauri/permissions/controller-commands.toml`.
- [ ] No secrets, personal data, or large binaries are committed.
- [ ] Every commit is signed off (below).
- [ ] The change is focused; unrelated cleanups go in separate PRs.

## Sign-off (Developer Certificate of Origin)

We use the DCO instead of a CLA. Add a sign-off line to every commit with `git commit -s`:

```
Signed-off-by: Your Name <you@example.com>
```

Use your real name and an address you control (a GitHub noreply address is fine). By signing off
you certify the following:

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

To sign off commits you already made: `git rebase --signoff origin/main`.

## AI-assisted contributions

Using AI tools is fine. You are responsible for what you submit: read and understand every line,
run the checks above, and only submit code you have the right to contribute under the DCO.
Do not paste text from other projects or from sources with incompatible licences. Disclose
significant AI assistance in the PR description if it affects how the change should be reviewed.
