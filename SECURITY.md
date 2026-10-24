# Security policy

## Supported versions

Only the latest released version receives security fixes. Until the first release, only the
current `main` branch is supported.

## Reporting a vulnerability

Please do not open a public issue for security problems.

1. Use GitHub's **private vulnerability reporting**: on the repository's *Security* tab choose
   *Report a vulnerability*.
2. If that is unavailable, email **[SECURITY CONTACT EMAIL: to be set by the maintainer before publication]**.

Please include the affected version, steps to reproduce, and the impact you expect. A sample
file that triggers the problem is very helpful (do not send files containing private data).

## What to expect

This is a volunteer-run project. Targets, not guarantees:

- Acknowledgement within 7 days.
- An initial assessment within 14 days.
- A fix or mitigation for confirmed issues, released as soon as practical, typically within 90 days.
- Credit in the release notes if you want it.

## Scope

In scope: the application and its build and release pipeline, for example:

- Memory-safety or logic flaws when importing `.pptx` or `.pdf` files (parsers, zip handling, size limits).
- The `sdmedia` file-serving protocol (path traversal, access outside the app data directory).
- The presentation window gaining the ability to call application commands or emit events.
- Tampering with released installers or CI workflows.

Out of scope: vulnerabilities in upstream projects that are not reachable through this application
(report those upstream), issues that need an already-compromised machine or administrator rights,
and denial of service by opening a very large file the application already rejects with an error.

## Bounty

There is no bug bounty.
