# Code signing policy

Windows installers of this application are code-signed so that users can verify who built them. This page
describes what is signed and who may approve it.

## Attribution

> **Applies only once SignPath signing is in use.** Until then, releases are signed with another certificate or not
> signed, and the line below does not apply.

Free code signing provided by SignPath.io, certificate by SignPath Foundation.

## What is signed

- Only release builds produced by this repository's GitHub Actions workflow (`.github/workflows/release.yml`),
  from a version tag (`v*`) on the default branch of this repository.
- Only artifacts built from the public source in this repository: the application executable and the installer.
- Third-party binaries bundled in the installer (for example PDFium and the WebView2 installer) keep their upstream
  signatures or are verified by checksum (`release/deps.json`); they are not re-signed under this project's identity.
- Builds from forks, pull requests, local machines or any other source are never signed.

## What is never signed

Malware, potentially unwanted programs, or software that circumvents the user's control. The application is
free software under GPL-3.0-or-later with no hidden functionality. It does not collect data or use the network
(see [privacy.md](./privacy.md)).

## Roles

| Role | Who | Responsibility |
|---|---|---|
| Committer | The maintainer, plus any contributor whose pull request is merged | Changes enter the repository only through reviewed pull requests |
| Reviewer | The maintainer | Reviews every change before it is merged |
| Approver | The maintainer | Approves each signing request, after checking that it comes from the release workflow for a tag on this repository |

At present all three roles are held by a single maintainer. When more maintainers join, this table will be
updated and the approver will not approve their own unreviewed changes.

## Process

1. A release is prepared by tagging a reviewed commit on the default branch.
2. The release workflow builds the installer on GitHub-hosted runners and submits it for signing.
3. The approver approves the signing request manually.
4. The signed installer is published as a GitHub release together with its SHA-256 checksum.

Signing keys are held by the signing service and are not accessible to repository contributors or workflow code.

## Privacy

This program will not transfer any information to other networked systems unless specifically requested by the user
or the person installing or operating it. See [privacy.md](./privacy.md).

## Reporting problems

If you believe a signed binary is malicious or was built improperly, report it as described in
[SECURITY.md](../SECURITY.md) so the certificate can be revoked if needed.
