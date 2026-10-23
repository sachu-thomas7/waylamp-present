# Owner answers and decisions (2026-10-23)

Answers to the open-question list, plus what the follow-up research found. Facts marked **[web]** come from search
results (several from third-party pages, so verify against the platform's own documentation before relying on them).
Nothing here edits the roadmap proposal; it feeds the next round of that review.

## Decided

| # | Topic | Decision |
|---|---|---|
| 11 | OS | **Windows first and only**; macOS/Linux promise nothing until Windows has run real services |
| 13 | Repo | Private now; **moves public once the roadmap is almost done**. Public + OSI licence unlocks free SignPath code signing (see "Signing") but also means choosing an OSI licence, which conflicts with `deny.toml`'s "private commercial app" wording: decide the licence before the move |
| 14 | Updates | **Manual** for now (no auto-updater); the church is on a closed network |
| 15 | Privacy flags | Documents are all public items; privacy flags are not needed yet |
| 16 | Live testing | The app is not used live until it is robust; a pre-service check is the gate |
| 17 | Hardware | Design for varied, current Windows machines: probe encoders at start, never assume NVENC |
| 18 | NDI runtime | Install it and test with NDI Tools test patterns first (next task) |
| 19 | Audio | Deferred. Accept any standard source later: OS audio devices, **NDI audio**, a phone app sharing sound over NDI |
| 7 | Reverse engineering the Mevo | **Not planned.** Mevo's own docs say full control is phone-app only; the iPad stays the operator tool |

## Findings that change the plan

- **Closed church Wi-Fi means offline machines.** Anything we ship must install without internet:
  - Use WebView2's **offline installer** (or the embedded bootstrapper) in our installer; the default bootstrapper needs internet.
  - **NDI runtime:** my earlier advice ("detect it and link to the download") assumed internet. NDI's licence allows redistributing the **NDI redistributable installer** inside our installer **[web: docs.ndi.video/docs/sdk/software-distribution]**, with conditions: keep the bundled version current, cover NDI's licence terms in our EULA or installer, show a link to ndi.video next to every place NDI is selected, and put NDI's trademark notice in the About box. NDI Tools (Studio Monitor etc.) may not be redistributed, only linked. Recommended: bundle the redistributable as an optional installer component, run it with its licence page shown (no `/verysilent`), and also detect an existing runtime via `NDI_RUNTIME_DIR_V*`. Needs a lawyer or at least a careful read of the EULA that ships with the SDK.
  - **Streaming needs an uplink.** The camera network may be closed; the laptop needs a second route to the internet (wired or hotspot) for Facebook and YouTube. Open question for the owner.
- **How churches use a Mevo Start [web]:** one Start, wireless receivers (Sennheiser) into its 3.5 mm input, NDI to OBS; the camera app on a phone for framing. Reported problem: choppy audio and video on Wi-Fi when that scene is active, which fits Mevo's own advice to use wired Ethernet on a dedicated network. Two consequences for us: (1) **wireless-mic audio arrives inside the NDI stream**, so F3.5 must treat NDI audio as the first audio source, not only OS devices; (2) test over Ethernet first and Wi-Fi second, and show a link-quality warning.
- **Mevo SRT:** Mevo's FAQ says all Mevo cameras support SRT, configured as a streaming destination in the app **[web]**. The owner has not seen the setting; enable and test it with the camera.
- **iPad and StreamDeck at once:** the owner reports the iPad can control the camera while another app receives the NDI video. Still worth one test: does NDI keep running after the iPad app closes or sleeps.

## Platform facts to verify (all **[web]**, mostly third-party)

| Platform | What was found | Our default | Action |
|---|---|---|---|
| YouTube | H.264 + AAC, CBR, keyframe every 2 s (never over 4 s), audio 128 kbps, RTMPS recommended; 720p30 about 3-8 Mbps, 1080p30 about 10-14 Mbps depending on source; limits of 10 active streams per channel and 3 per key | 2 s GOP, CBR, 128 kbps AAC, 1080p at 6 Mbps / 720p at 3 Mbps | Read YouTube's own encoder-settings page; try `rtmps://a.rtmps.youtube.com:443/live2` as the default (ours is plain `rtmp://`); consider a higher 1080p bitrate option |
| Facebook | Official Live Video API page says the streaming software takes the RTMP URL and key, about 4-5 s delay; third parties say 2 s keyframes, CBR, 4-6 Mbps, drops above about 9 Mbps; **conflicting reports on whether keys are one-time or persistent** | 6 Mbps cap is inside that range | Check Live Producer in the real account: is there a persistent key? That decides whether OAuth is worth building |
| Google OAuth | External apps in "Testing" status get refresh tokens that expire after 7 days; moving to "In production" may fix it but reports conflict; restricted scopes can need verification | n/a (paste-a-key first) | Spike with a real Google Cloud project before building anything |

## Signing and Windows security (item 20)

Goal: pass SmartScreen and antivirus as ordinary signed software, never "mark as safe".

1. **Now:** keep builds reproducible from CI, ship no runtime-downloaded executables, forbid `unsafe` (`#![forbid(unsafe_code)]`), keep the SBOM/notices plan, submit each release to Microsoft and Kaspersky as a false positive. Never tell users to add exclusions.
2. **Before public:** an OV code-signing certificate, or Azure Artifact Signing if the owner's location qualifies (individuals are limited to USA/Canada per the review).
3. **After the repo goes public:** SignPath Foundation's free signing. Requirements **[web: signpath.org/terms]**: an OSI-approved licence with no commercial dual-licensing, no proprietary code in the project (bundled system libraries are exempt), actively maintained, already released, automated build from the repo with origin verification. SmartScreen reputation still builds per file hash, so early releases may warn. Check SignPath's Tauri/NSIS notes at that time.
4. Windows trust basics for the installer: per-user NSIS install (no admin), signed installer and exe, stable publisher name, no self-modifying behaviour, WebView2 offline installer.

## Still open after these answers

1. Does NDI keep running when the iPad app is closed or asleep? (camera)
2. NDI|HX latency, `ndisrc` decode and smoothness, Ethernet versus Wi-Fi. (camera)
3. Mevo SRT: present in the app? (camera)
4. Does the laptop have an internet route separate from the closed camera network?
5. Which OSI licence for the public repo, and is that compatible with a commercial intent?
6. Facebook persistent key and Google production-mode token lifetime. (platform accounts)
7. Real YouTube and Facebook ingest test with the pasted keys. (stream keys)
