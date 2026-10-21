# OBS Studio code analysis: capture, compositing, transitions, output (for a Tauri/Rust/GStreamer presenter + camera compositor + streaming app)

Snapshot analysed: shallow clone of obs-studio, HEAD `95d5223b66da1c162acdb61c1bae2b9123a60f0e`, commit date `Tue Oct 20 2026 17:15:49 -0400`; `LIBOBS_API_VER` = 33.0.0 (`libobs/obs-config.h`).
Method: read-only reading of source. Citations are repo-relative paths (`REPO/` = the clone root) plus function/struct names, used in place of URLs. Everything below is **code-verified** unless prefixed `[INFERRED]` or `[NOT IN SNAPSHOT]`.
Snapshot caveat: submodules are NOT checked out. `plugins/obs-websocket/` and `plugins/obs-browser/` are empty directories (see `REPO/.gitmodules`), so obs-websocket code could not be read.

---

## 1. Capture: per-OS window/monitor capture, GPU texture vs CPU copy, cursor/border/permissions

### Takeaway
Every modern OBS capture path hands the compositor a GPU texture (or a GPU-importable handle: shared D3D11 texture, IOSurface, dmabuf, WGC frame surface). CPU copies exist only as explicit fallbacks (BitBlt/GDI, game-capture "shmem" mode, PipeWire MemPtr buffers). OBS never uses one method everywhere: it keeps several per OS and picks per target (by window class / battery+GPU state / support level), with a compatibility database for known-bad apps. Presentation apps (PowerPoint) are specifically routed to WGC.

### Cited Findings

**Windows: method selection**
- Window capture has three methods `METHOD_AUTO/BITBLT/WGC`. `choose_method()` returns BitBlt if WGC unsupported; in AUTO it returns WGC only for window classes in `wgc_partial_match_classes` ("Chrome", "Mozilla") and `wgc_whole_match_classes` (includes `PPTFrameClass`, `screenClass` = PowerPoint slideshow, `PodiumParent` = PowerPoint Presenter View, `XLMAIN`, `OpusApp`, `ApplicationFrameWindow`, `Windows.UI.Core.CoreWindow`, ...); everything else defaults to BitBlt. — `REPO/plugins/win-capture/window-capture.c` (`choose_method`, `wgc_whole_match_classes`)
- `wgc_supported` is set only if the graphics backend is D3D11 and Windows build >= 10.0.18362 (1903). — `REPO/plugins/win-capture/plugin-main.c` (`obs_module_load`, `win1903`)
- Monitor capture: on Win8+/D3D11 it registers `duplicator_capture_info`; default method DXGI Desktop Duplication. AUTO switches to WGC if the monitor has no DXGI index, or if on battery (`GetSystemPowerStatus`) with >=2 GPU adapters. — `REPO/plugins/win-capture/duplicator-monitor-capture.c` (`choose_method`); `plugin-main.c`
- A JSON compatibility DB (`compatibility.json`, schema-versioned, auto-updatable via `update_info_create`) surfaces warnings per exe/window class/capture method, e.g. PowerPoint "may not be capturable using ... BitBlt"; CS:GO/CS2 need `-allow_third_party_software` for Game Capture. — `REPO/plugins/win-capture/data/compatibility.json`, `compat-helpers.c` (`check_compatibility`), `window-capture.c` (`wc_check_compatibility`)

**Windows: WGC (Windows.Graphics.Capture), GPU path**
- Uses `Direct3D11CaptureFramePool` with **2 buffers**, `FrameArrived` callback; on each frame it `CopyResource`s (or `CopySubresourceRegion` for "client area" crop) the WGC surface into an OBS-owned `gs_texture` on the same D3D11 device, inside `obs_enter_graphics()`. A comment says "if they gave an SRV, we could avoid this copy" i.e. one GPU->GPU copy per frame remains. — `REPO/libobs-winrt/winrt-capture.cpp` (`winrt_capture::on_frame_arrived`)
- "need GetDesc because ContentSize is not reliable"; on size change it calls `frame_pool.Recreate(...)`; format mismatch (e.g. HDR toggled) sets `active=FALSE` to force re-init. HDR monitors use `DXGI_FORMAT_R16G16B16A16_FLOAT` (`get_pixel_format`). — same file
- Border: `IsBorderRequired(false)` after `GraphicsCaptureAccess::RequestAccessAsync(Borderless).get()` (blocking call), only if the property exists (`ApiInformation::IsPropertyPresent`). Cursor: `IsCursorCaptureEnabled` toggled only if supported; comment "disable cursor capture if possible since ours performs better" for monitor capture. — `winrt-capture.cpp` (`winrt_capture_init_internal`, `winrt_capture_border_toggle_supported`, `winrt_capture_cursor_toggle_supported`)
- Device loss: dedicated `winrt_capture_device_loss_release` closes the frame pool/session; item `Closed` event sets `active=FALSE`; monitor capture retries re-init every `RESET_INTERVAL_SEC` = 3.0s. — `winrt-capture.cpp`; `duplicator-monitor-capture.c`
- WinRT dispatcher is initialised on the **graphics thread** (`init_winrt_state`) so WGC works with libobs' render thread. — `REPO/libobs/obs-video.c` (`winrt_state`, `obs_graphics_thread`)

**Windows: DXGI Desktop Duplication, GPU path**
- `gs_duplicator_update_frame()`: `AcquireNextFrame(0, ...)` (non-blocking), `DXGI_ERROR_WAIT_TIMEOUT` => keep last frame, `DXGI_ERROR_ACCESS_LOST` => return false => capture torn down and re-created; `CopyResource` into an OBS texture, `ReleaseFrame()`. Uses `DuplicateOutput1` with a supported-format list (HDR) when available. — `REPO/libobs-d3d11/d3d11-duplicator.cpp`
- Cursor is composited separately: `cursor_capture()` via `GetCursorInfo`/`GetIconInfo` into a `GS_DYNAMIC` BGRA texture, drawn by `cursor_draw` at render time. — `REPO/plugins/win-capture/cursor-capture.c`, `duplicator-monitor-capture.c` (`draw_cursor`)

**Windows: GDI/BitBlt (CPU-backed but GPU-uploaded)**
- `dc_capture` renders window via `GetDC` + `BitBlt(... SRCCOPY)` into a `gs_texture_get_dc` GDI-compatible texture, i.e. a software capture. — `REPO/plugins/win-capture/dc-capture.c`

**Windows: Game Capture (DLL injection hook)**
- A hook DLL is injected into the game (`inject-helper`, separate 32/64-bit helper; optional "anti-cheat compatibility" hook via thread id), detours `IDXGISwapChain::Present/Present1/ResizeBuffers` (Detours: `DetourAttach`) plus D3D8/9/10/11/12/DDraw/GL/Vulkan hooks. — `REPO/plugins/win-capture/graphics-hook/dxgi-capture.cpp` (`hook_present`, `hook_resize_buffers`), `game-capture.c`
- Two transports: **shtex** (default): the hook copies the backbuffer into a `D3D11_RESOURCE_MISC_SHARED` texture and OBS opens it with `gs_texture_open_shared(tex_handle)` (GPU-only). **shmem** (fallback / "multi-adapter compatibility mode" / `!gs_shared_texture_available()`): hook `Map`s a staging copy and memcpy's into two double-buffered shared-memory textures guarded by named mutexes (CPU copy). — `graphics-hook/d3d11-capture.cpp` (`d3d11_shtex_init`, `d3d11_shmem_init`, `d3d11_shtex_capture`, line ~98 `MiscFlags = D3D11_RESOURCE_MISC_SHARED`); `game-capture.c` (`init_hook_info`, `init_shtex_capture`, `init_shmem_capture`, `copy_shmem_tex`)
- IPC objects: keepalive mutex, texture mutexes 1/2, hook-info file mapping, "hook rate" retry scaling (`HOOK_RATE_*`, `hook_rate_to_float`). Overlay capture optionally post-Present (`capture_overlay`). — `game-capture.c`; `dxgi-capture.cpp` (`capture_overlay` branches)
- Hook offsets are pre-computed per OS/driver by a helper (`get-graphics-offsets`) and cached. — `REPO/plugins/win-capture/get-graphics-offsets/`, `plugin-main.c`

**macOS: ScreenCaptureKit**
- `SCStream` with `SCContentFilter` for display (optionally excluding OBS's own app via `excludingApplications`), window (`initWithDesktopIndependentWindow`) or application capture. Frames arrive in `didOutputSampleBuffer`; the `CVPixelBuffer`'s **IOSurface** is retained (`IOSurfaceIncrementUseCount`) and bound zero-copy to a GPU texture on the render thread (`gs_texture_create_from_iosurface` first time, then `gs_texture_rebind_iosurface`). — `REPO/plugins/mac-capture/mac-sck-common.m` (`screen_stream_video_update`), `mac-sck-video-capture.m` (`init_screen_stream`, `sck_video_capture_tick`)
- Pixel format: `kCVPixelFormatType_64RGBAHalf` on macOS 15+, else `32BGRA`; `minimumFrameInterval` = 0.9 x OBS frame interval; `setShowsCursor:!hide_cursor`. Min OS: `API_AVAILABLE(macos(12.5))`, audio capture needs 13.0, some window/app features 14.2. — `mac-sck-video-capture.m` (`init_screen_stream`)
- Size changes mid-stream trigger `updateConfiguration:` with new width/height. `didStopWithError` handles `SCStreamErrorUserStopped` / `SCStreamErrorNoCaptureSource`, sets `capture_failed` and refreshes properties UI. — `mac-sck-common.m`
- Screen-recording permission is handled in the **frontend**, not the plugin: `CGPreflightScreenCaptureAccess()` / `CGRequestScreenCaptureAccess()` with a dedicated permissions dialog. — `REPO/frontend/utility/platform-osx.mm` (~L259-262), `REPO/frontend/dialogs/OBSPermissions.cpp`

**Linux: PipeWire portal capture**
- XDG ScreenCast portal over D-Bus (`org.freedesktop.portal.ScreenCast`): reads `AvailableCursorModes`; uses `PORTAL_CURSOR_MODE_METADATA` if offered, else EMBEDDED if cursor visible, else HIDDEN; `persist_mode=2` + `restore_token` stored in source settings (`RestoreToken`) so the user is not re-prompted. — `REPO/plugins/linux-pipewire/screencast-portal.c` (cursor mode and `restore_token` blocks ~L348-376, L256-262, L584)
- Buffers: negotiates DRM format + modifier lists from the GPU (`gs_query_dmabuf_capabilities`, `gs_query_dmabuf_modifiers_for_format`), accepts `SPA_DATA_DmaBuf` and imports with `gs_texture_create_from_dmabuf` (EGL `EGL_LINUX_DMA_BUF_EXT` image); if import fails it removes that modifier (`remove_modifier_from_format`) and renegotiates; fallback buffer type `SPA_DATA_MemPtr` => CPU upload via `gs_texture_create`. — `REPO/plugins/linux-pipewire/pipewire.c` (`on_param_changed_cb`, `on_process_cb`, `remove_modifier_from_format`, L~746-835, L975-979); `REPO/libobs-opengl/gl-egl-common.c` (`create_dmabuf_egl_image`)
- Cursor rendered by OBS from `SPA_META_Cursor` bitmap metadata; crop from `SPA_META_VideoCrop`; code comment: PipeWire does not reliably flag premultiplied alpha so OBS "just assume[s] premultiplied" (blend `ONE, INVSRCALPHA`), citing pipewire issue 3126. Version-gated features via `check_pw_version` (e.g. 0.3.24 dmabuf, 0.3.33 modifiers, 0.3.40 modifier removal). — `pipewire.c` (~L889-921, L1350-1368, `check_pw_version`)
- Portal-based capture is inherently user-mediated: no programmatic window enumeration/selection; every new source goes through the portal picker unless a restore token is valid. [INFERRED from portal API usage above]

**Camera / async video**
- Generic async sources (capture cards, webcams, NDI-like feeds) push CPU frames via `obs_source_output_video`; libobs buffers up to `MAX_ASYNC_FRAMES` = 30, timestamp-orders them, optionally `async_unbuffered` (low-latency) / `async_decoupled`, and uploads to textures with `update_async_textures` (YUV->RGB in shader). — `REPO/libobs/obs-source.c` (`obs_source_output_video2` path ~L3540-3625, `ready_async_frame` ~L4124, `update_async_textures` ~L2484), `obs-source.h`

### Inferences
- [INFERRED] For a PowerPoint/Keynote presenter app on Windows, WGC (not BitBlt, not hooks) is the path OBS itself picks for Office slideshow/presenter windows; it yields a GPU frame but still incurs one `CopyResource`. A Rust equivalent (windows-capture / `windows` crate `Direct3D11CaptureFramePool`) is a faithful pattern, with `FrameArrived` + 2-buffer pool.
- [INFERRED] Zero-copy into GStreamer is only realistic on the same D3D11 device (e.g. `d3d11` GStreamer elements wrapping the WGC texture) or macOS IOSurface (`GstGLMemory`/`applemedia`) or Linux dmabuf (`GstDmaBufAllocator`); OBS's per-OS "import native handle as texture" shape maps directly to that.
- [INFERRED] Copy OBS's "capability negotiation + graceful downgrade" idea (dmabuf modifiers -> remove failing modifier -> MemPtr; shtex -> shmem; WGC vs DXGI by condition) as an explicit fallback ladder per OS, and ship a data-file compatibility list.

### Gaps
- Did not read Linux X11 (`REPO/plugins/linux-capture/`, xshm/xcomposite) or Windows audio capture.
- Did not verify Windows version-specific WGC border behaviour (IsBorderRequired introduced in Win11/22H2 per Microsoft docs; code only feature-detects it, comment-free).
- Real-world performance numbers (ms/frame, CPU%) are not in the code; none claimed.
- macOS capture at pre-12.5 (CGDisplayStream) is a separate file (`mac-display-capture.m`) not analysed in depth.

---

## 2. Graphics layer: how sources render to textures, shared textures, GPU->CPU readback

### Takeaway
All sources render into one GPU "main texture" via a backend-neutral `gs_*` API (D3D11, OpenGL, and an alpha Swift Metal backend). Raw CPU readback is the exception and is minimised: the canvas is scaled and RGB->YUV converted on the GPU, then only the small NV12/I420 planes are copied to staging surfaces, double-buffered so the readback of frame N is mapped on frame N+1. When a hardware encoder supports it, readback is skipped entirely via shared NV12 textures.

### Cited Findings
- Constants: `NUM_TEXTURES 2` (staging double-buffer), `NUM_CHANNELS 3`, `NUM_ENCODE_TEXTURES 10`, `NUM_ENCODE_TEXTURE_FRAMES_TO_WAIT 1`. — `REPO/libobs/obs-internal.h` L49-53
- `render_video()`: `render_main_texture()` draws all registered draw callbacks then `obs_view_render()` into `video->render_texture` (can reuse another mix's already-rendered texture with same view/size/space: `can_reuse_mix_texture`); then `render_output_texture()` scales to output size (bilinear/bicubic/lanczos/area effect via `get_scale_effect_internal`; falls back to bilinear-lowres when output < half base; skips scale when within 16px: `resolution_close`); then `render_convert_texture()` converts to YUV planes in a pixel shader using `conversion_effect` (`color_vec0/1/2` matrix); then `stage_output_texture()` -> `gs_stage_texture` into `copy_surfaces[cur_texture]`. — `REPO/libobs/obs-video.c` (functions named above)
- `output_frame()`: renders frame N into slot `cur_texture`, then `download_frame()` maps the staging surface of `prev_texture` (frame N-1) with `gs_stagesurface_map`, i.e. **one frame of readback latency to avoid a GPU stall**; `gs_flush()` follows; mapped surface is unmapped next frame (`unmap_last_surface`). — `obs-video.c` (`output_frame`, `download_frame`, `unmap_last_surface`)
- CPU side after map: `output_video_data()` -> `video_output_lock_frame` -> memcpy planes (`set_gpu_converted_data`, handles I420/NV12/I444/I010/P010/P216/P416) into the `video-io` cache ring (max `MAX_CACHE_SIZE` 16), then per-consumer callbacks (CPU scaling/conversion only if an encoder wants a different format: `scale_video_output`). — `obs-video.c`; `REPO/libobs/media-io/video-io.c` (`video_output_cur_frame`, L158, `MAX_CONVERT_BUFFERS 3`)
- D3D11 staging surfaces are `D3D11_USAGE_STAGING` textures. — `REPO/libobs-d3d11/d3d11-stagesurf.cpp` L37, L62. OpenGL uses PBOs (`GL_PIXEL_PACK_BUFFER`, `GL_DYNAMIC_READ`) and, with a code comment that macOS only does async PBO transfers via FBO + `glReadPixels`. — `REPO/libobs-opengl/gl-stagesurf.c` (L28-35, L111-147)
- D3D11 shared textures: `GS_SHARED_KM_TEX` => `D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX`; `GS_SHARED_TEX` => `D3D11_RESOURCE_MISC_SHARED`. API: `gs_texture_get_shared_handle`, `gs_texture_open_shared`, `gs_texture_acquire_sync/release_sync` (keyed mutex `AcquireSync/ReleaseSync`). — `REPO/libobs-d3d11/d3d11-texture2d.cpp` L127-131, `d3d11-subsystem.cpp` (`device_texture_get_shared_handle`, `device_texture_acquire_sync`, L3216-3290). NV12/P010 are two-plane textures (`gs_texture_create_nv12/p010`).
- GPU scheduling: D3D11 backend calls `IDXGIDevice::SetGPUThreadPriority`; handles `DXGI_ERROR_DEVICE_REMOVED/RESET` with registered loss callbacks (`device_register_loss_callbacks`), sources like WGC release/re-init on loss. — `d3d11-subsystem.cpp` L452, L2416, L3395
- Native-handle import APIs: `gs_texture_create_from_iosurface` (`REPO/libobs/graphics/graphics.h` L823; GL impl `REPO/libobs-opengl/gl-cocoa.m` `CGLTexImageIOSurface2D`; Metal impl `REPO/libobs-metal/MetalTexture.swift` `bindSurface`), `gs_texture_create_from_dmabuf` (`gl-egl-common.c`), `gs_texture_open_shared` (D3D11). Syphon IOSurfaces with pixel format 0 are special-cased with a comment in `gl-cocoa.m` ~L485.
- Metal backend: `REPO/libobs-metal/README.md` states "alpha quality ... exclusive to Apple Silicon Macs", Metal 3 only, written in Swift with `@cdecl` exports; lists supported sources (SCK, media, browser, capture card), transitions (Cut, Fade, Stinger, Fade To Color, Luma Wipe, **not Slide/Swipe listed**), and a subset of filters.
- sRGB-aware/linear rendering and HDR (scRGB, `GS_CS_709_EXTENDED`, `GS_CS_709_SCRGB`) are threaded through render passes (`gs_enable_framebuffer_srgb`, `render_space`, `obs_get_video_sdr_white_level`). — `obs-video.c`, `obs-source-transition.c`

### Inferences
- [INFERRED] The key efficiency rules to copy: composite once at base size on the GPU; scale + colour-convert on the GPU before any readback; read back only the final YUV planes, ring-buffered (2+ slots) so you never block on the frame you just rendered; only read back at all if some consumer needs CPU data.
- [INFERRED] In GStreamer terms: `glvideomixer`/`d3d11compositor` (or `compositor` on CPU) -> `glcolorconvert`/`d3d11convert` -> hardware encoder elements that accept GPU memory (`nvh264enc`/`nvd3d11h264enc`, `qsvh264enc`, `amfh264enc`, `vah264enc`) keeps the same texture-to-encoder shape; `appsink`/`appsrc` readback should be limited to low-fps preview thumbnails.
- [INFERRED] Keyed-mutex hand-off (acquire with a key, release with the next key) is how OBS serialises two D3D11 devices (compositor device vs encoder device); GStreamer's d3d11 elements share a device context instead, which sidesteps this.

### Gaps
- Did not read `libobs-opengl/gl-subsystem.c` internals (FBO handling), Vulkan hook capture details, or `gs_texrender` implementation.
- Linux GL/EGL paths for sharing the composed texture with VAAPI/NVENC (CUDA-GL interop) only touched via the encoder files (section 3).

---

## 3. Video thread, pacing, frame drop/duplicate, texture encoders, encoder auto-selection and failure handling

### Takeaway
One dedicated graphics thread renders at a fixed interval using absolute-time scheduling; when it falls behind it computes how many frame intervals were missed and *duplicates the last frame to fill the timeline* (counting them as lagged) rather than speeding up. Encoders pull frames on their own threads; if the encoder cannot drain, new frames are *skipped* and counted. Hardware texture encoders (NVENC, AMF, QSV, VAAPI) take GPU textures directly. There is **no runtime encoder fallback chain**: selection is explicit in settings, a one-shot wizard picks a default, and a failed start surfaces an error to the user. The only automatic fallback is within a plugin (texture encoder -> its own non-texture variant).

### Cited Findings

**Pacing and drop/duplicate**
- Loop: `obs_graphics_thread_loop`: `tick_sources` -> `output_frames` (render + readback + encode hand-off) -> `render_displays` (previews) -> `execute_graphics_tasks` -> `video_sleep`. Thread named "libobs: graphics thread", profile root registered with the frame interval. — `REPO/libobs/obs-video.c` L1094-1199
- `video_sleep()`: `os_sleepto_ns(t)` to an *absolute* target; if the wake-up was on time, `count=1`; else `count = max(diff, interval)/interval` frames, `*p_time = cur_time + interval*count`. `total_frames += count; lagged_frames += count-1`. The `{timestamp, count}` `obs_vframe_info` is pushed to each active mix queue so the downstream knows to repeat the frame `count` times. — `obs-video.c` L805-861
- Duplicates in the raw path: `video_output_cur_frame()` repeats the cached frame `count` times (timestamp += frame_time per repeat). If the cache ring is full (encoder lagging), `video_output_lock_frame` returns false, adds `count` to `cache[last_added].count` and `.skipped`, and `skipped_frames` is incremented (log "skipped frames due to encoding lag"). — `REPO/libobs/media-io/video-io.c` (L126-188, `video_output_lock_frame` L509-545, `log_skipped` L451-462)
- Duplicates in the texture path: `queue_frame()` marks `tf->count++` on the last queued NV12 texture (no new render/copy), the encode thread re-encodes it, and decrements `count`; each repeat bumps `video_output_inc_texture_skipped_frames`. If no free texture (`gpu_encoder_avail_queue` empty) it duplicates instead of stalling. — `obs-video.c` L442-508; `REPO/libobs/obs-video-gpu-encode.c` L203-208
- Frame-rate divisors per encoder (`frame_rate_divisor_counter` explicit counter, comment: lets encoders started together start on same frame). — `obs-video-gpu-encode.c` L125-133; `video-io.c` L148-156

**GPU/texture encode pipeline**
- Pool of 10 shared NV12 (or P010) textures created with `GS_RENDER_TARGET | GS_SHARED_KM_TEX`; dedicated "obs gpu encode thread" woken by a semaphore; waits 1 frame at start (`wait_frames`); per encoder calls `encode_texture2(data, &tex, pts, lock_key, &next_key, &pkt, &received)`, chaining keyed-mutex keys across encoders; records per-frame CTS/FER/FERC timestamps for latency metrics. Swap-instead-of-copy trick: if only the texture path is active, the freshly converted texture is swapped into the queue and the pooled one reused (no `gs_copy_texture`). — `obs-video-gpu-encode.c` (`gpu_encode_thread`, `init_gpu_encoding`); `obs-video.c` `queue_frame` L475-493
- Encoder capability flags: `OBS_ENCODER_CAP_PASS_TEXTURE`, `DYN_BITRATE`, `INTERNAL`, `ROI`, `SCALING`, `MULTITRACK_DYN_BITRATE`, `DEPRECATED`. — `REPO/libobs/obs-encoder.h` L36-42. `gpu_encode_available()` requires PASS_TEXTURE and an active NV12/P010 mix. — `REPO/libobs/obs-encoder.c` L219-226
- NVENC (`obs_nvenc_{h264,hevc,av1}_tex`): `d3d11_encode` opens the shared handle on the encoder's D3D11 device (`OpenSharedResource`, cached per handle), `IDXGIKeyedMutex::AcquireSync(lock_key)` -> `CopyResource` into an NVENC-registered texture -> `ReleaseSync(next_key)` -> `nvEncMapInputResource`. Linux uses CUDA/OpenGL interop (`cuda_opengl_encode`). — `REPO/plugins/obs-nvenc/nvenc-d3d11.c` L188-260, `nvenc.c` L1404-1460
- AMF (`h264/h265/av1_texture_amf` with `*_fallback_amf` internal variants), QSV (`obs_qsv11_v2`, `obs_qsv11_av1`, `encode_texture2 = obs_qsv_encode_tex`, plus `_soft` variants), VAAPI (`vaapi_encode_tex`: exports VA surface as DRM PRIME, imports as dmabuf GL textures, `gs_copy_texture` per plane; non-texture `vaapi_encode_copy` fallback) — `REPO/plugins/obs-ffmpeg/texture-amf.cpp` L1802-1821, `REPO/plugins/obs-qsv11/obs-qsv11.c` L1293-1367, `REPO/plugins/obs-ffmpeg/obs-ffmpeg-vaapi.c` L401-450, L789, L1213-1261.
- VideoToolbox (macOS) is **not** a texture encoder: `.caps = DYN_BITRATE | MULTITRACK_DYN_BITRATE` only, `.encode = vt_encode` copies planes into a `CVPixelBuffer` (CPU memcpy, `CVPixelBufferLockBaseAddress`). — `REPO/plugins/mac-videotoolbox/encoder.c` L1122-1149, L1445-1450.
- NVENC in-plugin fallback: `nvenc_create_base()` reroutes to the `*_soft` (non-texture, `CAP_INTERNAL`) encoder when: user selected a different GPU than the render GPU (non-Linux), CPU scaling is on, NV12/P010 mix is not active, or `nvenc_create_internal` fails; "Already in non_texture encoder, can't fall back further!" is logged if the soft one fails. `obs_encoder_create_rerouted()` swaps `encoder->info` in place. — `REPO/plugins/obs-nvenc/nvenc.c` L925-995; `REPO/libobs/obs-encoder.c` L706-725
- Capability probing happens in an **out-of-process helper**: `nvenc_check()` runs `obs-nvenc-test(.exe)` via `os_process_pipe_create2`, parses an INI-like result (`nvenc_supported`, per-codec caps, driver/CUDA/NVENC versions), logs "NVENC test subprocess crashed" if it dies; the encoders are registered only if `nvenc_supported()`. AMF has an analogous `obs-amf-test`. — `REPO/plugins/obs-nvenc/nvenc-helpers.c` L286-380, `obs-nvenc.c` L15-20; `REPO/plugins/obs-ffmpeg/obs-amf-test/`

**Encoder "auto-selection" and startup failure (UI layer)**
- Simple output maps UI string -> encoder id in `get_simple_output_encoder()` (default `obs_x264`; NVENC prefers `obs_nvenc_h264_tex` if `EncoderAvailable()` else legacy `ffmpeg_nvenc`). `EncoderAvailable()` is just "is this id in `obs_enum_encoder_types`" i.e. whether the plugin registered it. — `REPO/frontend/utility/SimpleOutput.cpp` L90-125; `REPO/frontend/settings/OBSBasicSettings.cpp` L4881-4893
- The only "recommended encoder" logic is the **Auto-Configuration Wizard**: `AutoConfig::TestHardwareEncoding()` enumerates encoder ids to set `nvencAvailable/qsvAvailable/vceAvailable/appleAvailable`; `preferHardware` default = `nvencAvailable || appleAvailable || os_get_physical_cores() <= 4` (comment: newer NVENC has high quality-to-bitrate ratio). `TestStreamEncoderThread()` picks NVENC > QSV > Apple > AMD if hardware, else x264 after a software test; on Linux QSV is replaced by x264 ("CBR rate control is not guaranteed"). `FindIdealHardwareResolution()` probes resolution. — `REPO/frontend/wizards/AutoConfig.cpp` L183-236, `AutoConfigTestPage.cpp` L877-957
- Start failure path: `obs_output_start` -> `initialize_video_encoders` -> `obs_encoder_initialize` fails -> `obs_output_set_last_error(...)` -> start returns false -> `SimpleOutput::StartStreaming` logs "Stream output type ... failed to start! Last Error: ..." and returns false. No retry with another encoder. — `REPO/libobs/obs-output.c` L2578-2605; `REPO/frontend/utility/SimpleOutput.cpp` L749-775
- Mid-stream encode failure: `send_off_encoder_packet(success=false)` -> `full_stop(encoder)` -> `obs_output_force_stop` and a NULL packet sent to the output -> `OBS_OUTPUT_ENCODE_ERROR` -> modal "Output.StreamEncodeError" dialog (with last_error text). The stream is stopped, not migrated to x264. — `obs-encoder.c` L1372-1400, L1394-1400; `REPO/frontend/widgets/OBSBasic_Streaming.cpp` L321-323, L375-380

### Inferences
- [INFERRED] The duplicate-vs-skip policy is the important design to copy for a compositor: output clock is authoritative; if rendering is late, repeat frames to keep A/V timeline monotonic (OBS counts these as `lagged_frames`); if the *consumer* is late, drop and count (`skipped_frames`). Expose both counters in the UI.
- [INFERRED] OBS's own lack of automatic encoder fallback is a known gap you can beat: probe encoders out-of-process at startup (or at first run) using a tiny GStreamer test pipeline, store a ranked list (`nvd3d11h264enc` > `qsvh264enc` > `amfh264enc` > `vtenc_h264_hw` > `x264enc`), and on `Error` bus message during start auto-demote to the next one with a visible toast. This reuses OBS's good ideas (out-of-process probe, capability flags, in-plugin reroute) plus what it lacks.
- [INFERRED] Because OBS needs a keyed mutex to cross D3D11 devices and a copy into the encoder's own registered texture (NVENC), a GStreamer pipeline that keeps everything on one `GstD3D11Device` should be equally efficient with less machinery.

### Gaps
- Did not read QSV or AMF encode functions in detail (device/texture sharing mechanics) beyond caps/registration.
- No frame-time/latency measurements in the code; none claimed. The 1-frame readback latency is derived from the `prev_texture` logic only.
- Not verified: behaviour when the render GPU differs from the encode GPU for AMF/QSV (comments only in NVENC path).

---

## 4. Transitions and Studio Mode

### Takeaway
A transition in OBS is itself a *source* (type `OBS_SOURCE_TYPE_TRANSITION`) that holds two child sources A and B and is set as the program output (channel 0). `libobs` owns the generic machinery (render each child into its own offscreen texture, a normalised time `t` in [0,1] derived from the graphics-thread clock, audio crossfade callbacks); each transition plugin is a small shader + callback. Studio Mode adds a separate preview scene and a "Transition" button that starts the same machinery from preview to program.

### Cited Findings
- Seven built-in transitions registered: cut, fade, swipe, slide, stinger, fade-to-color, luma-wipe. — `REPO/plugins/obs-transitions/obs-transitions.c`. Fade is 148 lines: a `gs_effect` (`fade_transition.effect`, techniques `Fade`, `FadeSingle`, `FadeLinear`) with `tex_a`, `tex_b`, `fade_val`; callback `fade_callback(data, a, b, t, cx, cy)` draws one full-screen sprite. Linear-light blend when not in sRGB space, nonlinear blend in plain sRGB ("users want nonlinear fade"). — `plugins/obs-transitions/transition-fade.c`, `data/fade_transition.effect`
- Slide/swipe: UV offset in the pixel shader (`tex_a_dir`, `tex_b_dir`, `PSSlide`) with `cubic_ease_in_out` on `t`; the audio mix functions `mix_a/mix_b` also use the eased curve. — `transition-slide.c` L88-124, `transition-swipe.c`, `easings.h`, `data/slide_transition.effect`
- Cut = `obs_transition_video_render(source, NULL)` with no callback (instant), still goes through the same A/B plumbing. — `transition-cut.c`
- Core: `obs_transition_start(transition, mode, duration_ms, dest)`; mode `AUTO` (time based) or `MANUAL` (t-bar: `obs_transition_set_manual_time`, with spring "torque" smoothing `calc_torquef`). `calc_time()` = `(video_time - start_time) / duration`, clamped; **uses `obs->video.video_time`** (the graphics thread's scheduled clock), so the progress is deterministic with the render clock, not wall-clock UI timers. — `REPO/libobs/obs-source-transition.c` L197-215, L327-383, L446-470
- Rendering: each frame while active, `render_child()` renders source A and B into two `gs_texrender` RGBA (or float for HDR) offscreen textures sized to the transition (with per-child scale/alignment matrices, `OBS_TRANSITION_SCALE_*`), then calls the plugin callback with the two textures and `t`; when `t >= 1` it flips `transitioning_video=false` and `obs_transition_stop()` promotes B to A. If a child is missing it substitutes a transparent placeholder texture. Textures are `trylock`ed so the render thread never blocks. — `obs-source-transition.c` L614-762
- Audio crossfade uses per-sample `mix_a/mix_b` with sample-accurate time (`get_sample_time`, `process_audio`). — same file L855-1000
- Fixed-duration transitions: `obs_transition_enable_fixed(tr, true, duration)`; the stinger sets this from media length (`duration_ns`). Default transition duration 300 ms (`transition_duration` default in profile data). — `obs-source-transition.c` L1000-1012; `plugins/obs-transitions/transition-stinger.c` L201, L575; `REPO/frontend/widgets/OBSBasic_Transitions.cpp` L846, L1514
- Stinger: a private `ffmpeg_source` plays a video (optionally with a separate matte file or a packed track-matte layout, shader `stinger_matte_transition.effect`); the scene switch happens at `transition_point` (time ms or frame), `use_a = t < s->transition_point`; the stinger overlay is drawn over the chosen scene. Supports audio fade styles. — `transition-stinger.c` L63-165, L378-400, L524-585
- Frontend: program output is the transition object — `obs_set_output_source(0, transition)`; switching transition type does `obs_transition_swap_begin/end` to carry state, refuses to swap while active. Scene change calls `obs_transition_start(...)` with duration (global default, per-scene override transition+duration, quick-transition buttons, fade-to-black via `black` param with `prevFTBSource`) or `obs_transition_set` for a hard force. — `OBSBasic_Transitions.cpp` L295-440 (`TransitionToScene`), `SetTransition` L421-440
- Studio Mode (`previewProgramMode`): "scene duplication" mode makes a private duplicate of the scene (`obs_scene_duplicate(... OBS_SCENE_DUP_PRIVATE_REFS|PRIVATE_COPY)`) so edits to the preview scene don't affect program; `swapScenesMode` swaps preview/program after a transition; new transitions are blocked while `0 < t < 1` ("If actively transitioning, block new transitions"). — `REPO/frontend/widgets/OBSBasic_StudioMode.cpp` L184-225, L125-140; `OBSBasic_Transitions.cpp` L295-340
- The Metal backend README lists Cut, Fade, Stinger, Fade To Color, Luma Wipe but not Slide/Swipe as supported. — `REPO/libobs-metal/README.md`

### Inferences
- [INFERRED] For a presenter app, copy: transition = (A texture, B texture, `t`, shader) with `t` driven by the compositor's frame clock (not a JS timer); render both scenes to offscreen textures only while a transition is active; allow manual `t` (T-bar) and per-scene overrides; block re-entrancy while active; supply a "force" non-animated path.
- [INFERRED] In GStreamer this maps to a shader-based mixer (e.g. `glshader`/custom `GstGLBaseMixer` or `d3d11compositor` with per-pad alpha for fades; wipes need a shader). Pure alpha-fade is trivially `compositor` pad `alpha` animation via `GstController`/`GstInterpolationControlSource`; wipes/slides can be done by animating pad `xpos/ypos/width/height` with an easing function, avoiding custom shaders initially.
- [INFERRED] Slide transitions in a presentation context (slide-to-slide inside the deck) are a different problem from scene transitions; OBS has no concept of slides, so nothing to copy there.

### Gaps
- Did not read `transition-luma-wipe.c`/`luma_wipes` assets, `transition-fade-to-color.c` internals, nor the T-bar UI code beyond `obs_transition_set_manual_torque(.., 8.0f, 0.05f)` (frontend `OBSBasic_Transitions.cpp` L818).
- Studio-mode "edit properties mode" (private copy vs refs) semantics only skimmed.

---

## 5. Output and streaming: RTMP reconnect, dynamic bitrate, health indicators, WHIP, Enhanced RTMP / multitrack, SRT/RIST

### Takeaway
OBS separates *encoding* from *delivery*: encoded packets are pushed to an output, the output owns reconnect (core, shared) and congestion handling (protocol-specific). RTMP uses buffer-duration-based congestion: either drop frames by NAL priority (default) or, when the encoder supports it, lower the encoder bitrate (DBR). The UI health icon is just averaged `congestion` + dropped-frame percentage. WHIP exists but is thin (no OBS-side bandwidth adaptation visible in code). SRT/RIST are carried as MPEG-TS through FFmpeg-style custom protocol shims.

### Cited Findings

**Reconnect (core, shared by all outputs)**
- Defaults: `reconnect_retry_sec = 2`, exponential base `RECONNECT_RETRY_BASE_EXP = 1.5` + random jitter `rand_float(0)*0.05`, cap `RECONNECT_RETRY_MAX_MSEC = 15 min`. `output_reconnect()` multiplies the delay each try, stops after `reconnect_retry_max` and returns `OBS_OUTPUT_DISCONNECTED`; `can_reconnect()` only reconnects on DISCONNECTED (or while already reconnecting). An optional `reconnect_callback` can veto (used to stop retrying after N invalid-key errors). Signals `reconnect` / `reconnect_success` drive the status-bar countdown. — `REPO/libobs/obs-output.c` L32-33, L242-244, L2943-3030 (`output_reconnect`, `can_reconnect`, `obs_output_signal_stop`); `REPO/frontend/widgets/OBSBasicStatusBar.cpp` (`Reconnect`, `OBSOutputReconnect`)
- UI settings: `obs_output_set_reconnect_settings(streamOutput, maxRetries, retryDelay)`; `maxRetries=0` if reconnect disabled. — `REPO/frontend/utility/SimpleOutput.cpp` L741-746

**RTMP: congestion, frame dropping, dynamic bitrate (DBR)**
- Defaults: `drop_threshold_ms = 700` (B-frames) and `pframe_drop_threshold_ms = 900`, user range 200-10000; DBR constants `DBR_INC_TIMER = 4 s`, `DBR_TRIGGER_USEC = 200 ms`, `MIN_ESTIMATE_DURATION_MS = 1000`. — `REPO/plugins/obs-outputs/rtmp-stream.c` L44-46, L1905-1923
- `check_to_drop_frames()` measures `buffer_duration_usec = last_dts_usec - first_unsent_video_packet.dts_usec` over the send queue; `congestion = buffer_duration / drop_threshold`; if over threshold, `drop_frames()` drops queued packets whose `drop_priority < highest_priority` (never audio, never keyframes) and sets `min_priority` so subsequent low-priority packets are dropped until a higher-priority one arrives. NAL priorities: DISPOSABLE 0, LOW 1, HIGH 2, HIGHEST 3 (keyframe). — `rtmp-stream.c` L1564-1620, L1763-1830; `REPO/libobs/obs-nal.h` L27-30; `obs-avc.c` L103
- DBR (`OPT_DYN_BITRATE`): enabled only if all video encoders have `OBS_ENCODER_CAP_DYN_BITRATE` (multitrack: `MULTITRACK_DYN_BITRATE`). Each sent packet's `{size, send_beg, send_end}` feeds `dbr_add_frame`, giving an *achieved-throughput* estimate over a sliding window (audio bitrate subtracted, floor 50 kbps). When queued buffer >= 200 ms, `dbr_bitrate_lowered()` snaps to the estimate (rounded to 100 kbps) and calls `obs_encoder_update` with the new bitrate (`dbr_set_bitrate`, using an interpolation table for multi-rung multitrack). Recovery: every 4 s add 10% of original (`dbr_inc_bitrate`) up to original; on reconnect, bitrate resets to original. With DBR on, frame dropping is replaced by bitrate lowering ("let's test without dropping frames at all first" comment). — `rtmp-stream.c` L588-616 (`dbr_add_frame`), L1623-1700, L1366-1465 (capability gating), L1785-1815
- Optional "new socket loop" backend: congestion becomes `write_buf_len / write_buf_size` (send-buffer fill) instead of queue-duration. `rtmp_stream_congestion()`. — `rtmp-stream.c` L1966-1975, L1053-1059 (`ideal_buffer_size = total_bitrate*128`, min 128 KiB)

**Health indicators in UI**
- Status bar shows `obs_output_get_frames_dropped / get_total_frames` percent (`DroppedFrames` string) and a 4-level icon from `obs_output_get_congestion()`, smoothed (`avg of current & last`, take max if rising, clamp to 1), averaged over `congestionUpdateSeconds = 4` samples; thresholds excellent 0.0, good 0.3333, mediocre 0.6667, bad 1.0. Disconnected state handled separately. — `REPO/frontend/widgets/OBSBasicStatusBar.cpp` L9-13, L325-385
- Error taxonomy returned by outputs: `OBS_OUTPUT_BAD_PATH, CONNECT_FAILED, INVALID_STREAM, ENCODE_ERROR, HDR_DISABLED, ERROR, DISCONNECTED` mapped to localized dialogs; `get_last_error` text appended. — `REPO/frontend/widgets/OBSBasic_Streaming.cpp` L296-345

**Enhanced RTMP / multitrack video**
- FLV muxing implements Enhanced RTMP/FLV: ex-header packet types (`PACKETTYPE_SEQ_START/FRAMES/FRAMESX/MULTITRACK`, audio `MULTICHANNEL_CONFIG/MULTITRACK`), FourCC codecs (`av01`, `hvc1`), `MULTITRACKTYPE_ONE_TRACK/MANY_TRACKS/MANY_TRACKS_MANY_CODECS`, and `PACKETTYPE_FRAMESX` as the zero-composition-time-offset optimisation. HEVC/AV1 over RTMP only via these ex-headers. — `REPO/plugins/obs-outputs/flv-mux.c` L45-65, L146-160, L425-515; `rtmp-stream.c` L1383 (`is_multitrack` detection); `rtmp_output_info.encoded_video_codecs = "h264;hevc;av1"`.
- "Multitrack video" ("Enhanced Broadcasting"): frontend POSTs a client-capabilities JSON to a service-provided "go live"/auto-config URL (`GoLiveApi::PostData`: OBS version, supported codecs h264/h265/av1 enumerated from registered encoders, GPU adapter index, canvas sizes, fps, audio params, `maximum_aggregate_bitrate`, `maximum_video_tracks`), receives ingest endpoints (+ optional stream key via `authentication`) and `encoder_configurations` (per-track resolution/bitrate, `bitrate_interpolation_points`, `canvas_index`), then builds N video encoders in one `obs_encoder_group` on one `rtmp_output` (+ optional parallel `flv`/`mp4` recording output sharing the encoders). On any config failure it throws a warning and **falls back to the normal single-stream path** ("FailedToStartStream.FallbackToDefault"). It sets a reconnect callback that gives up after `MAX_RECONNECT_ATTEMPTS` invalid-key errors and uses `RestartOnError()` to re-run the whole go-live negotiation. — `REPO/frontend/utility/MultitrackVideoOutput.cpp` L45-80, L134-152, L470-500, L595-660, L870-890; `GoLiveAPI_PostData.cpp`, `GoLiveAPI_Network.cpp`
- GPU scaling for extra tracks: `obs_encoder_set_gpu_scale_type` + core "video mix" per distinct resolution (`maybe_set_up_gpu_rescale` in `obs-encoder.c`), reusing the already-rendered base texture when geometry matches (`can_reuse_mix_texture` in `obs-video.c`).

**WHIP (WebRTC) output**
- `plugins/obs-webrtc/whip-output.cpp` (792 lines) uses **libdatachannel** (`rtc::PeerConnection`, `rtc::Track`, packetizers). Video codecs `h264;hevc(if enabled);av1`, audio Opus only; `OBS_OUTPUT_ENCODED | SERVICE | MULTI_TRACK_AV`. Signalling: HTTP `POST` of the SDP offer (`Content-Type: application/sdp`, optional `Authorization: Bearer <token>`, 8 s timeout, redirects followed), reads `Location` header for the resource URL, parses `Link` headers for STUN/TURN ICE servers (`ParseLinkHeader`), applies answer, `gatherLocalCandidates(iceServers)`; stop sends HTTP `DELETE` to the resource URL. Maps failures to `CONNECT_FAILED`, `INVALID_STREAM` (HTTP auth), `BAD_PATH`, `ENCODE_ERROR`. PeerConnection `Disconnected` -> `OBS_OUTPUT_DISCONNECTED` (so core reconnect applies), `Failed` -> `OBS_OUTPUT_ERROR`. Reliability features: `RtcpNackResponder` (video NACK buffer 4000), RTCP sender reports, and a `PacingHandler` (`video_bitrate*10000`, 5 ms) on the video packetizer. Simulcast layer negotiation present (`simulcast_layers_in_answer`). — `REPO/plugins/obs-webrtc/whip-output.cpp` L27, L149-260, L282-340, L342-391, L399-560, L616-660, L695-790
- A grep for `twcc|remb|congestion|goog` in `plugins/obs-webrtc/*.cpp` returns nothing: **no sender-side bandwidth estimation / bitrate adaptation is implemented in the OBS plugin code** (any adaptation would have to come from libdatachannel, which this snapshot does not include).

**SRT / RIST**
- Implemented as MPEG-TS over custom protocol shims `libsrt_*`/`librist_*` plugged into an FFmpeg `AVIOContext` (`avio_alloc_context` with `libsrt_write`/`librist_write`). SRT options exposed: `latency`, `rcvlatency`, `peerlatency`, `tlpktdrop`, `maxbw`, `streamid`, passphrase/`pbkeylen`, `SRTO_TRANSTYPE`, connect timeout, payload size default 1316 (`SRT_LIVE_DEFAULT_PAYLOAD_SIZE`). RIST: profile MAIN, buffer 3000 ms default, periodic stats JSON logged. — `REPO/plugins/obs-ffmpeg/obs-ffmpeg-mpegts.c` L280-400; `obs-ffmpeg-srt.h` L30-70, L328-373; `obs-ffmpeg-rist.h` L36-160

### Inferences
- [INFERRED] RTMP congestion model worth copying: measure *send-queue duration* (time between oldest unsent video DTS and newest), not just socket errors; compute congestion = duration/threshold in [0,1]; act in two tiers (drop disposable frames first, then bitrate reduction if the encoder can change bitrate live). With GStreamer, `x264enc`/`nvh264enc` accept live `bitrate` property changes, so DBR is directly portable; expose `appsink`/queue `current-level-time` as the congestion signal.
- [INFERRED] For your streaming health UI, adopt: dropped-frames % + 4-state congestion + reconnect countdown + explicit error enum, as OBS does. Note your CLAUDE.md reserves red/green/amber for streaming health; OBS also uses a green/yellow/orange/red icon set (`excellentPixmap...badPixmap`, content not read here).
- [INFERRED] WHIP in GStreamer: `whipclientsink` (gst-plugins-rs) already includes transport-cc-based congestion control hooks via webrtcbin; OBS's thin plugin suggests WHIP quality depends on the underlying lib's congestion control, so verify gst-plugin-webrtc's `cc` behaviour rather than assuming parity (this repo's F2.4 already runs WHIP ingest for phones).

### Gaps
- `obs-websocket`, SRT/RIST reconnect behaviour inside the ffmpeg-mpegts output (not read), and `obs_output_*` delay/"stream delay" feature (`obs-output-delay.c`) were not analysed.
- Icon pixmap semantics (colors) not read.
- libdatachannel version/licensing and its congestion-control capabilities not in this repo.
- Not verified: actual RTMPS/TLS stack or TCP tuning beyond `SO_SNDBUF` and the "new socket loop" option.

---

## 6. Remote control (obs-websocket)

### Takeaway
obs-websocket is a **core module built into OBS** (`add_core_module(obs-websocket)`) but its source lives in a git submodule that is empty in this snapshot, so its API surface and auth could not be verified from code here.

### Cited Findings
- `plugins/CMakeLists.txt` macro `check_obs_websocket()` hard-errors ("Required submodule 'obs-websocket' not available") if `plugins/obs-websocket/CMakeLists.txt` is missing, otherwise `add_core_module(obs-websocket)`; invoked at L108. — `REPO/plugins/CMakeLists.txt` L37-42, L108
- Submodule URL `https://github.com/obsproject/obs-websocket.git` at path `plugins/obs-websocket`. — `REPO/.gitmodules`
- `plugins/obs-websocket/` is an empty directory in the clone (verified by listing).
- The frontend exposes a plugin-facing C API `obs_frontend_*` (events such as `OBS_FRONTEND_EVENT_STREAMING_STARTED`, scene/transition/stream control) in `REPO/frontend/api/obs-frontend-api.h`; obs-websocket and other plugins drive the app through this API (a comment in `REPO/frontend/widgets/OBSBasic.cpp` L922 notes obs-websocket depends on profile file locations). — same files.

### Inferences
- [INFERRED, from background knowledge not this snapshot] obs-websocket v5 speaks JSON (and MessagePack) request/response/event messages over WebSocket, default port 4455, with an optional password using a salted SHA-256 challenge/response in the `Hello`/`Identify` handshake and event-subscription bitmasks. Treat this as unverified here; fetch the obs-websocket repo/protocol doc for citations before putting it in a report.
- [INFERRED] The architectural lesson that IS verifiable: OBS exposes control via a stable frontend C API + event enum, and the remote protocol is just a plugin on top, so the internal API (scenes, transitions, streaming state) was designed to be driven programmatically. For your app the analogue is your `AppEvent` bus + Tauri commands; a WebSocket/OSC remote layer should be another subscriber, which fits invariant 2 (event bus only).

### Gaps
- Entire API surface (requests list, events, batch requests), auth details, and security defaults: not available in snapshot. Requires a separate fetch of `obsproject/obs-websocket` (GPL-2.0 per its repo, to be confirmed).

---

## 7. Complexity, licensing, and structural patterns worth adopting

### Takeaway
OBS is GPL-2.0(-or-later) throughout the core; copying code would force your app under GPL. Study and reimplement the ideas from your own GStreamer/Rust building blocks. The structural patterns (registered "info structs" with capability flags; sources/filters/transitions as one type; out-of-process capability probing; a stable frontend API; data-file compatibility DB) are worth adopting as designs.

### Cited Findings
- Top-level license is GNU GPL v2 (`REPO/COPYING`); sampled source headers state "either version 2 of the License, or (at your option) any later version" (e.g. `REPO/libobs/obs-video-gpu-encode.c`, `libobs/obs-video.c`-era headers, `plugins/linux-pipewire/pipewire.c` L7-8). Individual deps/plugins may carry other licenses (e.g. `deps/`, `plugins/obs-websocket` submodule); not audited here.
- Scale: single files are large (e.g. `libobs/obs-video.c` 1200 lines, `libobs/obs-source-transition.c` 1072, `libobs/graphics` + three backends, `plugins/obs-webrtc/whip-output.cpp` 792, `plugins/linux-pipewire/pipewire.c` 1471, `plugins/obs-outputs/rtmp-stream.c` >2000). ~55 plugin dirs under `REPO/plugins/` plus three graphics backends (`libobs-d3d11`, `libobs-opengl`, `libobs-metal`) and a Windows WinRT helper lib (`libobs-winrt`).
- Plugin boundary: shared libraries export `obs_module_load` etc. via `OBS_DECLARE_MODULE()` with an API version check (`obs_module_ver` returns `LIBOBS_API_VER`); modules register typed info structs (`obs_register_source/encoder/output/service(&info)`). — `REPO/libobs/obs-module.h` L54-201; e.g. `obs-transitions.c` `obs_module_load`.
- One `struct obs_source_info` serves inputs, filters, transitions and scenes: `.type = OBS_SOURCE_TYPE_{INPUT,FILTER,TRANSITION,SCENE}`, flag bits (`OBS_SOURCE_VIDEO`, `AUDIO`, `ASYNC_VIDEO`, `COMPOSITE`, `DO_NOT_DUPLICATE`, `SRGB`, ...), callbacks `create/destroy/update/video_tick/video_render/audio_render/video_get_color_space`. — `REPO/libobs/obs-source.h` L34-37, L88-204, L222+.
- Same pattern for encoders (`obs_encoder_info` with `caps`, `encode`, `encode_texture2`, `get_video_info`), outputs (`obs_output_info` with `flags`, `protocols`, `encoded_video_codecs`, `get_congestion`, `get_dropped_frames`) and services. — `obs-encoder.h`, `plugins/obs-outputs/rtmp-stream.c` `rtmp_output_info`, `plugins/obs-webrtc/whip-output.cpp` `info` struct.
- Effects are data files in HLSL-like `.effect` shaders compiled per backend (`gs_effect_create_from_file`, techniques/passes), so transitions/filters are portable across D3D11/GL/Metal. — `plugins/obs-transitions/data/*.effect`
- Main-thread/Qt UI vs render thread isolation: UI calls into libobs; outputs signal via calldata signals (`stream_starting`, `reconnect`, ...); UI marshals with `QMetaObject::invokeMethod`. — `frontend/widgets/OBSBasicStatusBar.cpp`, `OBSBasic_Streaming.cpp`

### Inferences
- [INFERRED] GPL-2.0 implications: do not copy or closely translate OBS code (including shaders, the compatibility JSON entries, or the graphics-hook) into your Tauri app unless you accept GPL-2.0-or-later for the combined work; reading to learn algorithms/constants and writing clean-room Rust is fine from a copyright standpoint (facts and ideas are not copyrightable; thresholds like 700/900 ms are your own choices to tune). Not legal advice; get a license review if distributing commercially.
- [INFERRED] Patterns to adopt (as designs): (1) one output-clock-driven frame loop with a `{timestamp,count}` repeat protocol; (2) capability flags on encoders + out-of-process probing + deterministic ranked fallback; (3) source/filter/transition as one trait (`Source` with `kind`) in Rust with a `Transition` trait taking `(a_tex, b_tex, t)`; (4) congestion as a normalised 0..1 signal feeding both UI and bitrate control; (5) a data-driven compatibility list updated out-of-band; (6) per-OS capture backends behind one `CaptureBackend` trait with an auto-select function and fallback chain.
- [INFERRED] Things to avoid: heavy global state and giant files; OBS's manual per-vendor encoder plumbing (GStreamer already abstracts encoders); no auto-fallback on encoder start failure; invasive DLL-injection game capture (irrelevant for a presenter app and AV-hostile, compare CLAUDE.md's AV warnings); blocking `RequestAccessAsync(...).get()` in WGC init (could freeze a UI thread: run it off the UI thread in Rust).

### Gaps
- No license audit of `deps/` or per-plugin license headers beyond samples; no review of third-party licenses (libdatachannel, librist, libsrt, ffmpeg) that OBS links.
- Plugin ABI stability beyond the `LIBOBS_API_VER` check not analysed.

---

## Appendix: key file index (all relative to `REPO/`)
- Video loop/pacing/readback: `libobs/obs-video.c`, `libobs/media-io/video-io.c`, `libobs/obs-video-gpu-encode.c`, `libobs/obs-internal.h`
- Encoders/outputs core: `libobs/obs-encoder.c`, `libobs/obs-encoder.h`, `libobs/obs-output.c`, `libobs/obs-nal.h`
- Transitions: `libobs/obs-source-transition.c`, `plugins/obs-transitions/*.c`, `plugins/obs-transitions/data/*.effect`, `frontend/widgets/OBSBasic_Transitions.cpp`, `frontend/widgets/OBSBasic_StudioMode.cpp`
- Windows capture: `plugins/win-capture/{window-capture,duplicator-monitor-capture,game-capture,cursor-capture,dc-capture,plugin-main}.c`, `plugins/win-capture/graphics-hook/*`, `libobs-winrt/winrt-capture.cpp`, `libobs-d3d11/{d3d11-duplicator,d3d11-texture2d,d3d11-stagesurf,d3d11-subsystem}.cpp`, `plugins/win-capture/data/compatibility.json`
- macOS: `plugins/mac-capture/mac-sck-{video-capture,common}.m`, `frontend/utility/platform-osx.mm`, `libobs-opengl/gl-cocoa.m`, `libobs-metal/*`
- Linux: `plugins/linux-pipewire/{pipewire,screencast-portal}.c`, `libobs-opengl/gl-egl-common.c`
- Encoders: `plugins/obs-nvenc/{nvenc,nvenc-d3d11,nvenc-helpers}.c`, `plugins/obs-ffmpeg/{texture-amf.cpp,obs-ffmpeg-vaapi.c}`, `plugins/obs-qsv11/obs-qsv11.c`, `plugins/mac-videotoolbox/encoder.c`
- Frontend encoder selection/errors: `frontend/utility/SimpleOutput.cpp`, `frontend/wizards/AutoConfig*.cpp`, `frontend/widgets/OBSBasic_Streaming.cpp`, `frontend/widgets/OBSBasicStatusBar.cpp`
- Output protocols: `plugins/obs-outputs/{rtmp-stream,flv-mux}.c`, `plugins/obs-webrtc/whip-output.cpp`, `plugins/obs-ffmpeg/obs-ffmpeg-{mpegts.c,srt.h,rist.h}`, `frontend/utility/{MultitrackVideoOutput,GoLiveAPI_*}.cpp`
- Not present in snapshot: `plugins/obs-websocket/` (empty submodule), `plugins/obs-browser/` (empty submodule).
