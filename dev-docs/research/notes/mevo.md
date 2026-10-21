# Mevo (Logitech Mevo cameras + Mevo Studio / Multicam software) as a reference

Research date: 2026-10-21. Method note: several Mevo help-center pages returned HTTP 403 to fetch; those facts come from search-result summaries of the pages (marked "via search snippet"). Treat those as medium confidence.

## 1. Product line and status as of 2026 (Start, Core, Multicam, Studio; platforms; discontinuation)

### Takeaway
"Mevo Multicam" and "Mevo Studio" are the same mobile app (same App Store ID 1503021034); it appears to have been renamed Mevo Studio, and it is at v4.2.4 (Sept, year not stated on page). There is no desktop Mevo Studio; platforms are iOS/iPadOS/visionOS and Android only. I found no evidence of discontinuation of Start/Core or the app.

### Cited Findings
- Mevo Studio on the App Store: v4.2.4 (28 Sept), Logitech Inc., free with in-app purchases, iPhone iOS 16+, iPad iPadOS 16+, visionOS 1.0+; Mac not listed — [App Store](https://apps.apple.com/ba/app/mevo-studio/id1503021034)
- The same App Store ID 1503021034 is the old "Logitech Mevo Multicam" listing, and Android package is `com.mevo.multicam` (APK listings show "Mevo Studio" 4.0.2, 4.0.3, 4.1.2, 3.6.x) — [App Store (old name)](https://apps.apple.com/py/app/logitech-mevo-multicam/id1503021034?l=en-GB), [APKMirror 4.1.2](https://www.apkmirror.com/apk/logitech-europe-s-a/logitech-mevo-multicam/mevo-studio-4-1-2-release/). Mevo's site and EU site use the "Mevo Studio" name; Logitech's 2024 Core press release still says "Mevo Multicam" — [Business Wire](https://www.businesswire.com/news/home/20240319877017/en)
- Studio is described as compatible with Mevo Go app, Mevo Core, Mevo Start, Mevo Plus and first-gen Mevo (via search snippet) — [Mevo Pro page](https://www.mevo.com/pages/pro)
- Mevo Core: announced March 2024, $999, body-only Micro Four Thirds, 4K30, Wi-Fi 6E — [Engadget](https://www.engadget.com/logitechs-999-4k-livestreaming-camera-is-triple-the-price-of-its-1080p-model-070146814.html), [Mevo Core page](https://www.mevo.com/products/mevo-core)
- Mevo Core specs: 4/3 CMOS 8.3MP; 4K30 record (MP4, 40 Mbps), stream up to 4K30 at 12 Mbps; Wi-Fi 6E, BT 5.1, 2x USB-C (one UVC webcam), 1x HDMI out, 3.5mm TRS in, microSD; NDI, HDMI, SRT, RTMP output; H.264/HEVC; AAC; ~6h battery; Ethernet not listed — [Mevo Core page](https://www.mevo.com/products/mevo-core)
- Mevo Start: 1080p predecessor, 3-pack was $999 at launch (TechCrunch 2021); Start spec sheet: streaming up to 1080p30 at 15 Mbps — [TechCrunch](https://techcrunch.com/2021/11/09/mevo-multicam-hands-on/), [Start datasheet](https://www.logitech.com/content/dam/logitech/en/education/pdf/mevo-start-datasheet.pdf) (via search snippet; PDF unreadable)
- Logitech business page markets "Mevo for Business" with multi-camera control from phone/tablet — [Logitech](https://www.logitech.com/en-ca/business/mevo-for-business.html)

### Inferences
- Active development: releases through Aug-Sept (4.2 added Browser Sources; 4.2.2 added 1080p60 NDI/SRT) suggest the app is maintained, not sunset. Year of those releases is inferred as 2026.
- A replacement app should treat "Mevo Studio" as the current product name and "Multicam" as legacy naming users still search for.

### Gaps
- No official statement on Logitech strategy, discontinuation or end-of-life for Mevo Start/Plus/1st gen found. Mevo Plus/1st gen appear legacy.
- No desktop (Windows/macOS) Mevo Studio found; App Store lists no Mac. Not 100% ruled out (Apple Silicon "iPad app on Mac" availability unconfirmed).
- Exact release year of 4.x versions not confirmed.

## 2. Features: multicam, scenes, graphics, audio, destinations, recording, remote, NDI/HDMI/USB

### Takeaway
Studio is a phone/tablet-based vision mixer: cameras send video over the local network to the mobile device, which composes scenes/graphics, mixes audio, encodes, and streams. Free tier covers multicam sync, switching, graphics and one destination; Pro adds multistream, Mevo Go sources, and NDI|HX in/out of Studio.

### Cited Findings
- Features: switch among multiple Mevo cameras plus Mevo Go, NDI, UVC/webcam sources; pan/zoom/crop each view; stream to YouTube/Facebook/Twitch etc.; record program to device and per-camera ISO to each camera's microSD; graphics, lower thirds, countdowns, presentations, media; Scene creator; Auto-Director; audio mixer; per-camera exposure/color — [App Store](https://apps.apple.com/ba/app/mevo-studio/id1503021034)
- Recent versions: 4.2 (13 Aug) updated NDI library + Browser Sources for web/URL graphics; 4.2.1 SRT settings moved into camera settings; 4.2.2 NDI and SRT support 1080p60; 4.2.4 improved physical keyboard input and Ethernet options in setup — [App Store](https://apps.apple.com/ba/app/mevo-studio/id1503021034)
- Free: Studio download, syncing multiple cameras, switching/layering/graphics, per-camera audio mixing, streaming to one favorite destination. Pro: multistream (powered by Streamlabs), Mevo Go app (phones/tablets as sources), NDI|HX input and output with Studio. Outputting NDI|HX directly from an individual camera is free — [Mevo Pro page](https://www.mevo.com/pages/pro)
- Camera count: Mevo's pages state no maximum; a help article says if you can't add more than three cameras, update the app, and stream warnings usually mean the phone lacks processing power for the camera count (via search snippet) — [Mevo help](https://help.mevo.com/hc/en-us/articles/34373606581396-Mevo-Studio-How-many-cameras-can-I-use). The Start 3-pack is the marketed multicam kit — [Engadget](https://www.engadget.com/the-mevo-start-3-pack-tiny-powerful-multicam-streaming-studio-133046416.html)
- Camera-side outputs (via search snippet): HDMI output is Core only; NDI works on Start and Core only; SRT on all Mevo cameras; Plus and 1st gen do not support NDI but can do webcam, RTMP, SRT; USB-C UVC webcam mode — [Mevo help: output via HDMI/SRT/NDI/UVC](https://help.mevo.com/hc/en-us/articles/34335555416980-How-to-output-your-Mevo-Camera-via-HDMI-SRT-NDI-or-UVC-Webcam), [Mevo help: OBS](https://help.mevo.com/hc/en-us/articles/34372194485268-Connecting-Mevo-Cameras-to-OBS)
- OBS: official methods are RTMP, NDI (needs OBS NDI plugin + NDI Tools), wired/wireless webcam, SRT (via search snippet) — [Mevo help: OBS](https://help.mevo.com/hc/en-us/articles/34372194485268-Connecting-Mevo-Cameras-to-OBS)
- Multicam prep/fade: prepare a shot on one camera and fade between them, zoom, overlays, digital panning — [TechCrunch](https://techcrunch.com/2021/11/09/mevo-multicam-hands-on/)
- Recording: program recorded on phone; per-camera to microSD at 1080p — [Logitech business page](https://www.logitech.com/en-ca/business/mevo-for-business.html), [App Store](https://apps.apple.com/ba/app/mevo-studio/id1503021034)

### Inferences
- "Presentations" in Studio is a basic slide/media overlay feature; no evidence of ProPresenter-class slide control (not documented in sources found).
- The existence of per-camera NDI/SRT/RTMP/HDMI/UVC output means a replacement app could ingest Start/Core cameras via NDI|HX, SRT or UVC without the proprietary protocol, but with feature loss (see section 3).

### Gaps
- Official hard maximum on cameras unknown; sources only imply device-bound (and "three" historically).
- Specific scene/PiP layout list, slide-format support (PPTX/PDF?) and remote-control (second device operator) behavior not found.
- Latency numbers not found (Engadget only mentions high vs low latency stream modes).

## 3. Technical transport details, ports, APIs, third-party integration

### Takeaway
Cameras do not stream directly to the internet in multicam mode: they send "raw" footage over the local network to the mobile device, which encodes the program. Ports are documented; no public API, RTSP, or community integrations (GitHub/Companion/vMix) were found.

### Cited Findings
- Data path: each camera sends raw footage through the local network to the device running Studio; the device encodes per shot selections and sends the program to platforms (via search snippet) — [Mevo help: network config](https://help.mevo.com/hc/en-us/articles/360058647952-Multicam-Configuration-and-Setup)
- Ports listed: TCP 38000, TCP 38001, UDP 61000-63000 for camera/app; streaming ports TCP 1935, 80, 443 (via search snippet) — same article
- Network modes: Wi-Fi, camera Access Point, Ethernet (article title); setup needs Wi-Fi and Bluetooth enabled on the device; each camera needs separate firmware update over Bluetooth — [Mevo help](https://help.mevo.com/hc/en-us/articles/360058647952-Multicam-Configuration-and-Setup), [Engadget](https://www.engadget.com/the-mevo-start-3-pack-tiny-powerful-multicam-streaming-studio-133046416.html)
- Start can create own Wi-Fi (access point) and stream via LTE when run in hotspot mode from a phone running the app; spec sheet recommends min steady 6 Mbps upload (another site says 3 Mbps; sources conflict) — [Start datasheet](https://www.logitech.com/content/dam/logitech/en/education/pdf/mevo-start-datasheet.pdf), [Gadgeteer](https://the-gadgeteer.com/?p=292393) (via search snippet)
- No official RTSP output found; Mevo OBS doc lists RTMP/NDI/webcam/SRT only. An OBS forum thread shows users asking to point Mevo RTMP at a local server — [OBS forum](https://obsproject.com/forum/threads/using-external-ip-cam-like-meveo-directly-in-obs.96932)
- Studio embeds NDI library (updated in 4.2) and Browser Sources — [App Store](https://apps.apple.com/ba/app/mevo-studio/id1503021034)

### Inferences
- The camera-to-app transport (ports 38000/38001 TCP + UDP 61000-63000) is proprietary and undocumented; a third-party app cannot replicate Studio's control/multicam sync without reverse engineering. Standard outputs (NDI|HX on Start/Core, SRT on all, RTMP, UVC, HDMI on Core) are the only interoperable routes.
- No cellular bonding; LTE is single-link phone hotspot.

### Gaps
- No public API/SDK, GitHub projects, Bitfocus Companion or vMix modules found for Mevo (absence of evidence from limited searches).
- SRT mode (caller/listener) and NDI|HX version/bitrate/discovery behavior for cameras not confirmed.
- Whether camera control (PTZ-like crop, exposure) is available over any standard protocol: not found.

## 4. Complaints, feature requests, replacement requirements

### Takeaway
Main complaints: connection stability, laborious firmware/setup, dependence on phone encoding power, paywalled multistream/NDI, missing features versus the old Mevo app. Review sources are dated (2021) or aggregator-based.

### Cited Findings
- TechCrunch (2021): setup frustrating, all three cameras needed firmware updates taking hours and several phone restarts; reviewer could not yet trust it for important streams — [TechCrunch](https://techcrunch.com/2021/11/09/mevo-multicam-hands-on/)
- Engadget (Start 3-pack): app fussy; switching apps or locking the screen killed the stream; unexplained drops — [Engadget](https://www.engadget.com/the-mevo-start-3-pack-tiny-powerful-multicam-streaming-studio-133046416.html)
- Aggregator user reviews: can't multistream in the Multicam app, missing pan/zoom (early versions), SRT request, notifications interrupting broadcasts — [Justuseapp reviews](https://justuseapp.com/en/app/1503021034/logitech-mevo-multicam/reviews) (low-reliability aggregator; ~49 reviews on [marlvel](https://marlvel.ai/apps/mevo-studio))
- No HDMI out on Start/Plus (Core only) — [Mevo help](https://help.mevo.com/hc/en-us/articles/34335555416980-How-to-output-your-Mevo-Camera-via-HDMI-SRT-NDI-or-UVC-Webcam)

### Inferences
- Must-haves for replacement to keep Mevo cameras usable: NDI|HX ingest (Start/Core), SRT listener/caller ingest, RTMP ingest, UVC; accept that proprietary multicam features (crop/pan per camera, sync, exposure, camera setup/firmware over BT) won't be available without Studio.
- Differentiators: desktop-class encoding (not phone-limited), stable background operation, no paywall for multistream/NDI, slide integration.

### Gaps
- No Reddit/forum threads retrieved; recent (2025-26) complaint evidence is thin.

## 5. Pricing / subscription

### Takeaway
Studio is free; Mevo Pro is $19/month or $149/year (14-day trial). Streamlabs in-app plans appear separately in the App Store.

### Cited Findings
- Mevo Pro: $19/month, $149/year (35% off), 14-day trial both, cancel anytime — [Mevo Pro](https://www.mevo.com/pages/pro)
- App Store in-app purchases (BA storefront, EUR): Streamlabs Annual €199, Monthly €29 — [App Store](https://apps.apple.com/ba/app/mevo-studio/id1503021034)
- Hardware: Mevo Core $999 body-only — [Engadget](https://www.engadget.com/logitechs-999-4k-livestreaming-camera-is-triple-the-price-of-its-1080p-model-070146814.html); Start 3-pack $999 (2021 launch) — [TechCrunch](https://techcrunch.com/2021/11/09/mevo-multicam-hands-on/)

### Inferences
- The Streamlabs-denominated IAP prices differ from the $19/$149 on mevo.com; possible regional/tier difference or a different SKU. Unverified which is current.

### Gaps
- Current 2026 hardware street prices for Start/Plus not verified.
