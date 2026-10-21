# ProPresenter (Renewed Vision) — reference profile as of October 2026

Evidence quality note: official pages were partly unreachable (several 404/401/522). Much of the detail below comes from search-tool summaries of official/retailer/review pages, not full reads. Items are marked [documented] (vendor source) or [reported] (reviews/forums/retailers). Many core-feature details (Looks, Masks, Messages, Timers, Scripture/CCLI, MIDI/OSC, Alpha Keyer, ProPresenter Cloud, Keynote/ProPresenter 6 import) were NOT directly verified this session and are listed under Gaps.

## Feature inventory (presentation model, outputs, inputs, control, import, streaming)

### Takeaway
ProPresenter is a Mac/Windows presentation + light-production tool: multi-screen render engine with SDI/NDI/Syphon outputs, props/stage/masks layers, video inputs, built-in RTMP stream/record (since 7.1), an official REST-style network API (since 7.9), a Stream Deck plugin, and a native PowerPoint importer (21.2). Verified detail on the presentation data model (libraries/playlists/arrangements/Looks) was not obtained.

### Cited Findings
- Version line: 7.x (7.0–7.16.3) then 17, 18 (18–18.4), 19, 20, 21 (21.0.1–21.4.2), and 22 in beta dated Oct 19, 2026 [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes)
- Outputs: render engine drives multiple simultaneous outputs incl. SDI, NDI, Syphon, plus HDMI/DisplayPort; SDI/NDI/Syphon screens cannot be toggled off once created; screens configured in Configure Screens window [reported, retailer/forum summary] — [Pro7 User Guide / retailer summary](https://files.renewedvision.com/propresenter/support/Pro7UserGuide.pdf), [CCI listing](https://shop.ccisolutions.com/StoreFront/product/renewed-vision-propresenter-7)
- Blackmagic: UltraStudio, DeckLink, ATEM and Thunderbolt-enclosure cards supported; key/fill needs compatible Blackmagic hardware; alpha keying only on NDI outputs and SDI on alpha-capable hardware [reported] — same sources as above
- A dedicated (non-integrated) GPU is advised for more than 4 HD screens [reported] — same sources
- Layered content model: backgrounds, live video, props, masks controlled independently [reported]; Prop Collections, Auto-clear Props, Single Prop Mode, TCP/IP communication, user accounts arrived in v17 [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes)
- v18: Windowed Output (adjustable floating output window, e.g. for video meetings), AirCast Receiver (AirPlay screens/audio as video/audio inputs), updated editor toolbar, macOS Sequoia support [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes), [Download/feature page](https://renewedvision.com/propresenter/download)
- Stage: stage layouts and Stage Screens with current/next slide datalinks [documented] — [Download/feature page](https://renewedvision.com/propresenter/download)
- Live video: webcams/virtual inputs [reported]; AirCast Receiver inputs [documented]; v22 adds HLS stream URL playback for video/audio actions [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes)
- Streaming/recording: 7.1 (2020) added RTMP/RTMPS streaming and local recording of any configured output screen or a dedicated stream screen (e.g. with logo bug); can do both concurrently; codec availability depends on OS (PC lacks some options, HAP alpha suggested) [documented/trade press] — [Vendor blog](https://renewedvision.com/blog/propresenter-7-1-the-live-stream-update), [Vendor blog 2](https://renewedvision.com/blog/?p=214223), [AV Network](https://www.avnetwork.com/news/renewed-vision-adds-live-streaming-recording-to-propresenter-with-71-update)
- Import: native PowerPoint importer added in 21.2 (Feb 3, 2026) imports .pptx as editable presentations without PowerPoint installed; v22 can skip background on import [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes), [Vendor blog](https://renewedvision.com/blog/your-pastor-sends-you-a-powerpoint)
- Official public API introduced in 7.9: enable Network in preferences, default port 50000, optional password; OpenAPI/Swagger spec published [documented] — [API docs](https://renewedvision.com/propresenter/api/), [Swagger](https://renewedvision.com/api_spec/index.html), [community mirror](https://github.com/jeffmikels/ProPresenter-API)
- Community also reverse-documented older undocumented `/remote` and `/stagedisplay` WebSocket protocols, plus Pro7 file format and Network Link in separate repos [reported] — [jeffmikels repo](https://github.com/jeffmikels/ProPresenter-API)
- Third-party MCP server wraps the API with reported 231 endpoints in 27 groups (indicates breadth of API) [reported] — [alxpark/propresenter-mcp](https://mcpservers.org/servers/alxpark/propresenter-mcp)
- 7.9 also added official Stream Deck plugin (next/prev slide, clear, groups, macros, props); Renewed Vision notes Bitfocus Companion as a third-party option [documented] — [Support article](https://support.renewedvision.com/hc/en-us/articles/5284750340371-Setting-Up-The-ProPresenter-Stream-Deck-Plugin)
- ProPresenter Remote (current, included with active subscription): stage mode with notes/timers, macros, props, audio playlists, messages, libraries/playlists; Remote Classic no longer sold, works up to v20 [documented] — [Support](https://support.renewedvision.com/hc/en-us/articles/43163349444755-ProPresenter-Remote-Classic), [Remote page](https://renewedvision.com/propresenter/remote)
- v22 beta: cloud workspaces with near-real-time sync, Text Animations as builds, Stickers (ProContent), Amazing curriculum import, export to ProDisplay, screens can be disabled to save render resources [documented, beta] — [Release notes](https://renewedvision.com/propresenter/release-notes)
- Workspaces: vendor warns against keeping workspaces in OneDrive/iCloud/Dropbox/Google Drive (missing media, "Downloading Remote Content") [documented] — [Support](https://support.renewedvision.com/hc/en-us/articles/45416857033363-How-to-Prevent-ProPresenter-Files-from-Syncing-to-Cloud-Services)
- Other recent: Planning Center songs keep custom sequence on import (21.4); custom key mappings for slide views (21.3); Blackmagic Desktop Video 15.3 SDI crash fix (21.0.1) [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes)

### Inferences
- The stream/record path is output-screen based (re-encodes a rendered screen), not a multi-source switcher; ProPresenter appears to be an output renderer plus control surface rather than a production switcher.
- API + Stream Deck + Companion + MCP wrappers suggest a competitor should ship an equivalent documented control API from day one; interop (matching/accepting ProPresenter API shapes) could ease migration.
- Heavy reliance on dated 2020 streaming sources means current streaming capability (v21/22) is unconfirmed.

### Gaps
- Not verified: libraries/playlists/presentations/groups/arrangements details, Looks, masks, messages, timers/clocks, scripture/CCLI, MIDI/OSC support, Alpha Keyer details, ProPresenter Cloud (pre-v22) details, Keynote and ProPresenter 6 import, screen/window capture sources. Pro7 User Guide PDF fetch failed (HTTP 522).
- The release-notes page labels 21.x "Beta"; unclear whether that is a page artifact. No 2024 release dates/notes were obtainable; v19/v20 contents unverified.
- No verified Bitfocus Companion ProPresenter module page (existence of a module is common knowledge but unsourced here).

## Pricing/licensing and platforms

### Takeaway
Now subscription-first (monthly/yearly per seat, sign-in replacing unlock codes) with a legacy perpetual license that keeps working on the last version available when ProPresenter+ maintenance lapses. macOS and Windows only; no Linux.

### Cited Findings
- Subscription plans monthly/yearly, add/remove seats, sign-in instead of unlock codes [documented] — [Subscription page](https://renewedvision.com/propresenter/subscription-learn-more)
- Reseller listing: Standard $29/seat/month or $289/seat/year; 20-seat Campus (houses of worship) $59/month or $649/year; vendor legacy page reportedly shows the same yearly figures [reported, conflicting with older figures] — [Search summary of vendor legacy page](https://www.renewedvision.com/propresenter/migrate-your-license), [GetApp](https://www.getapp.com.au/software/118675/propresenter)
- Older perpetual model: from about $399 single seat incl. 1 year ProPresenter+; renewals reportedly $189 single / $469 Campus (possibly outdated) [reported] — [Vendor support article](https://support.renewedvision.com/hc/en-us/articles/360052854233-Is-there-a-subscription-or-annual-fee-ProPresenter-Explanation), [GetApp](https://www.getapp.com.au/software/118675/propresenter)
- Perpetual license stays usable on the last version available at ProPresenter+ expiry [documented via search summary] — [Support article](https://support.renewedvision.com/hc/en-us/articles/360052854233-Is-there-a-subscription-or-annual-fee-ProPresenter-Explanation)
- Platforms: macOS (v22 adds macOS 27) and Windows [documented] — [Download page](https://renewedvision.com/propresenter/download); a listing claims separate licenses per OS (reported, unverified) — [Fitgap](https://us.fitgap.com/products/propresenter)

### Inferences
- Move to subscription is a likely pain point for small churches; a one-time-price or free tier is a differentiation angle.

### Gaps
- Current exact prices/tiers unconfirmed (vendor pages 404/401); Linux status confirmed only by absence.

## UX praise and recurring complaints

### Takeaway
Reviews are mostly positive on reliability in weekly services but recurring complaints cover Windows update regressions, Windows being a port feeling worse than Mac, resource use, learning curve, cost/subscription, and multi-display failures on Mac.

### Cited Findings
- Windows update instability forced a rollback (volunteer sound engineer) [reported] — [Capterra reviews](https://www.capterra.com/p/170172/ProPresenter/reviews/?page=3)
- 2019 review: happy on Mac, not on Windows [reported, dated] — [Capterra](https://www.capterra.com/p/170172/ProPresenter/reviews/?page=3)
- Reviewer: Windows version feels like a Mac port with crash/lag/UI issues; switched tools [reported] — [Software Advice](https://www.softwareadvice.com/worship/propresenter-profile/)
- Resource-intensive on older machines, especially video playback; steep learning curve; high cost [reported] — [Capterra](https://www.capterra.com/p/170172/ProPresenter/reviews/?page=5)
- Positive: "very stable", rarely crashes, weekly services run smoothly [reported] — [Capterra](https://www.capterra.com/p/170172/ProPresenter/reviews/?page=5)
- r/ProPresenter post: Mac Studio with four displays plus capture card randomly fails to drive all outputs; separate post about flicker from 720 vs 1080 frame-rate mismatch [reported] — [r/ProPresenter mirror](https://redlib.hbubli.cc/r/ProPresenter)
- Competitors named: OpenLP, FreeShow (stage display, remote control), EasyWorship, WorshipTools Presenter, Faithlife Proclaim [reported] — [AlternativeTo](https://alternativeto.net/software/propresenter/)

### Inferences
- PowerPoint fidelity was historically weak (native importer only in 21.2, 2026), consistent with the importer being a headline feature.

### Gaps
- Praise for live editing/quick looks not verified; PowerPoint-fidelity complaints, licensing complaints beyond reviews, and Windows vs Mac parity beyond a few anecdotes not sourced; forum/Facebook/YouTube sources not reached.

## Recent direction (2024-2026) and automation surface

### Takeaway
Direction is subscription/cloud (workspaces, cloud sync in v22 beta, accounts), content ecosystem (ProContent stickers, curriculum import), importer and output-resource improvements; automation via official API, Stream Deck plugin, Remote app and community wrappers.

### Cited Findings
- 21.0.1 (Nov 26, 2025), 21.1 (Dec 9, 2025), 21.2 (Feb 3, 2026), 21.3 (Mar 18, 2026), 21.4 (Jul 1, 2026), 22 beta (Oct 19, 2026) — features as listed above [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes)
- No plugin/extension SDK found; automation is macros, network API, TCP/IP comms (v17) and MIDI-triggered macros (21.1 fix mentions macro MIDI triggering) [documented] — [Release notes](https://renewedvision.com/propresenter/release-notes)

### Inferences
- Roadmap emphasizes cloud/content services over production features.

### Gaps
- 2024 releases (v18–19) dates and features unconfirmed; no OSC confirmation.

## Gaps a presentation + production product could target

### Takeaway
Sources suggest ProPresenter streams/records a rendered output screen but show no multi-camera switching or composited scene editor; capture of live video exists only as inputs/layers.

### Cited Findings
- Streaming is RTMP/RTMPS of a configured output screen; recording to local disk; no mention of multi-source switching, WHIP, or multi-destination restreaming in found sources [documented, absence-based] — [Vendor blog](https://renewedvision.com/blog/?p=214223)
- Recording the program output was suggested via third-party Syphon Recorder/NDI Video Monitor in a tutorial [reported] — [Search summary of vendor tutorials](https://renewedvision.com/propresenter/tutorials/)
- Platform limited to macOS/Windows; no Linux [documented by absence] — [Download page](https://renewedvision.com/propresenter/download)

### Inferences
- Opportunities: true multi-camera switching with NDI/WHIP phone ingest, composited scenes, WHIP/multi-destination egress, Linux support, cheaper/perpetual licensing, strong PowerPoint/PDF import, documented API plus Companion/Stream Deck parity. All are inferred from absence in limited sources; verify against current v21/22 docs before relying on them.

### Gaps
- Absence claims unverified against full docs; ProPresenter's actual current limits on camera inputs, multiple-destination streaming, and encoder options not found.
