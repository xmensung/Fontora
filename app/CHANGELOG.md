# Changelog

All notable changes to ZFontManager are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [0.10.0] — 2026-11-03

### 新增

- **缺字检测。** 顶栏筛选菜单里的「只看不缺字」，以及每个字体卡片右下角的
  「缺 N 字」标记。判定不在前端做：`check_glyphs` 命令把预览文本拆成码点，
  逐字问字体文件的 cmap，把 `GlyphId(0)`（.notdef 空盒子）也算作缺 —— 有 cmap
  条目却指向空字形的残字体，这样才不会蒙混过关。缺的字在预览里标红。

  判定结果按「字面 → 码点 → 有无」缓存在 Rust 侧，同一个字只读一次盘；换预览
  文字才需要重新读，重复打开软件不再重扫。重扫字体目录时整批作废。

- **预览文本预设与历史。** 输入框左侧的 `T` 按钮打开预设菜单：现代文、繁体、
  标点符号、偏旁部首、拉丁字母、日文假名、生僻字、数字符号。每组下面附一行
  灰色小字预览实际内容。自己敲的句子在停手 900ms 后进「最近用过」，最多 10 条，
  可一键清空。

- **行高调节与竖排预览。** 字号滑杆右侧新增 `⌄`，展开「行高」（1.00–2.40）
  与「排列」（横排 / 竖排）。竖排用 `writing-mode: vertical-rl` +
  `text-orientation: mixed`，中文标点按竖排规则旋转；没有 `vert` 特性的字体会
  把标点挤在字框中间，这一眼就能看出字库做没做竖排。

- **中文书体筛选。** 筛选菜单新增一组：黑体 / 宋体 / 楷体 / 仿宋 / 圆体 / 隶书 /
  篆书 / 行草 / 其他中文。之前只有西式的衬线 / 无衬线 / 等宽，思源宋体、苹方、
  微软雅黑全挤在两个桶里，「只看楷体」根本做不到。判定只看族名关键词
  （仿宋先于宋、行草先于楷），判不出来的落到「其他」。

- **叠加对比。** 对比面板顶部新增「并排 / 逐行 / 叠加」三种模式。叠加模式把各款
  字体同一段文字绝对定位重合，浅色主题用正片叠底、深色主题用滤色，重合处自然
  加深 / 提亮；配字号与浓度滑杆，画面可直接拖动对位，「重置」回到原点。

- **OpenType 特性可视化。** 详情页新增第 05 节：把 `get_features` 读到的特性标签
  做成开关，常用的（连字、自由连字、小型大写、旧式数字、分数…）排在前面，
  点开后直接作用在下方样张上（`font-feature-settings`）。挂名没实现的特性
   一眼能看出来 —— 开关点了没反应就是没做。

- **青瓷皮肤。** 深色为窑变、浅色为粉青，釉青主色 + 一线水光。设置 → 皮肤里
  与墨韵、霓虹并列。

### 变更

- **主色从「科技紫」换成藕荷。** 深色 `#e092b4 → #b96e93`，浅色
  `#b04a72 → #8b3a5b`。原先的紫是渐变工具默认值那一挂，和字体工具的气质不搭；
  而且紫色与「红=错误 / 黄=警告」之外没有一个专属的语义位置，用完只剩显眼。
  藕荷是降过饱和的玫红，只出现在选中态、进度条、开关这类小面积上。

  同时新增 `--on-accent`（落在主色块上的字色）、`--accent-rgb`（供各处透明叠加
  直接引用，不再硬编码色值）。浅色主色配深字、深色主色配白字，不再一律用白 ——
  17 处原本写死 `color: white` 的主色块都改成了这个令牌。

- **默认预览文字改成中文。** 原来是 `The quick brown fox jumps over the lazy dog`，
  在中文界面里像没做完。现在默认「春风又绿江南岸，明月何时照我还。」，
  想换英文点预设里的「拉丁字母」即可。

### 修复

- **自动标签建议里中文字体永远判不成「中英混排」。** `suggestTags` 查的是
  `scripts.includes("Han")`，但 `parser::script_coverage` 报的键名是 `cjk`，
  这个判断从来没命中过。同一处 `"Latin"` 也是同样的问题。

## [0.9.0] — 2026-11-03

### 新增

- **标签体系与打标签界面。** 31 个预置标签分成三组 —— 书体源流（宋体 / 仿宋 / 黑体 /
  楷体 / 圆体 / 隶书 / 篆书 / 行书 / 草书 / 西文衬线 / 西文无衬线 / 手写体 / 装饰体）、
  功能与气质（正文 / 标题 / 书法手写 / 艺术展示 / 品牌海报 / 屏幕 UI / 印刷出版 /
  复古怀旧 / 商务严肃 / 活泼儿童）、特殊属性（等宽 / 斜体 / 中英混排 / 可变字体 /
  繁体支持 / 生僻字支持 / 大字库 / 专业排版）。分组只是前端常量，存储层仍是
  「家族名 → 标签数组」，旧标签无需迁移，认不出分组的自动归入「其他」。

  详情页标签区按分组顺序聚拢，每个标签带 `×`；`+` 展开词表面板，可点选也可自由输入。
  批量打标签共用同一套词表，弹窗顶部写明「将 N 个标签应用到 M 个字体」。
  侧栏标签分组折叠、带计数，右键可「从所有字体移除该标签」。

- **按标签筛选。** 顶栏筛选菜单新增标签区：分组 chips 多选（并集）、「未打标签」、
  筛选生效时出现「清空标签筛选」。侧栏点标签直接进入该标签的浏览视图。

- **自动标签建议。** 详情页按字体文件的客观元数据推导建议标签：族名书体关键词、
  字重阈值（≥700 标题、≥800 品牌海报）、斜体 / 等宽 / 可变、脚本覆盖、汉字字数
  ≥6000 算大字库、含扩展 B 区算生僻字支持、smcp / onum / frac 算专业排版。
  建议以 ✦ 浅色标签呈现，点一下采纳即从建议中消失，已有手动标签一律保留。

- **AI 名录导出与回读。** 导出中心新增「AI 名录」，导出全量元数据 + 现有标签的 JSON；
  侧栏「回读 AI 名录…」支持 `{家族名: [标签]}`、`[{family, tags}]` 以及导出中心生成的
  清单三种格式（按 family 自动聚合）。只改文件里提到的家族，未提到的原样不动；
  某家族给空数组即清空其标签；标签逐项 trim + 去重后写入。

- **墨韵与霓虹两套皮肤。** 设置 → 皮肤，另加原本的紫调玻璃，共三档。

### 变更

- **默认浅色主题。** 首帧前就把主题定成浅色，不再先闪一下深色再变白。

## [0.8.1] — 2026-10-25

### 修复

- **自定义字体库文件夹无法移除。** 设置 → 字体库文件夹里选定自定义目录后，那一行
  只有路径、没有移除入口，「使用自定义文件夹」关掉之后路径仍然留着，删不掉也换不回
  默认目录。现在该行右侧多了一个 `×`：点一下即清空路径并把开关关掉，回到默认字体库
  目录；「监听的文件夹」的移除按钮也补上了悬停底色和提示，不再是一个不显眼的叉。

- **浅色主题发灰。** 浅色下大量表面沿用了深色主题的「墨色叠层」（`rgba(墨色, 4~7%)`），
  叠在浅灰画布上就成了一片脏灰：卡片和背景糊在一起，设置面板是奶灰色，输入框和
  菜单更暗。这次把浅色主题的表面语义改成「抬高往白走」：

  - 画布收到 `#f4f5f9`，卡片 / 列表 / 菜单 / 对话框一律用纯白 + 细描边 + 轻投影；
  - 设置面板从「白 58% + 30% 灰 + 高亮模糊」改成近不透明的白色厚面板；
  - 搜索框、开关轨道、分段控件去掉黑色内阴影，用的灰也更浅；
  - 遮罩从 35~65% 的深灰压到 22%，不再把整屏糊成水泥色；
  - 背景紫蓝光晕在浅色下减半，避免全屏泛出一层灰雾。

  深色主题的取值一处未动。

## [0.7.2] — 2026-10-15

### Fixed

- **The Affinity connection status in Settings did not refresh while the panel
  was open.** It showed whatever it found the moment you opened Settings, so
  during the startup grace period added in 0.7.1 it could sit on
  "Offline · Affinity link failed..." long after Affinity had connected. That
  wording is identical to one of the errors from #16, which made a working
  installation look like a failed one. The panel now follows the link live.

- **Opening Settings could connect to Affinity during its startup.** The status
  check ran a full connection of its own, outside the grace period that keeps
  the watcher away from Affinity while it starts — so opening Settings at the
  wrong moment could trigger the very crash 0.7.1 fixes. The watcher now
  publishes what it already knows, and the panel reads that instead of opening
  a connection. The *Test connection* button still probes on demand, as before.

  Found while testing 0.7.1 on Windows.

## [0.7.1] — 2026-10-15

### Fixed

- **Affinity could be taken down at startup by the font activation
  integration.** Affinity's MCP server starts accepting connections before the
  application has finished initialising, and ZFontManager connected to it the
  instant the process appeared — often while Affinity was still on
  "Initializing...". The request landing in that window could close Affinity
  with no error dialog, and left ZFontManager showing
  "preamble read failed: error decoding response body" or a failed link to
  `localhost:6767/sse`.

  ZFontManager now leaves Affinity alone for the first 20 seconds after it
  appears, and if the link still fails it retries with a widening gap (10s,
  20s, 40s, up to two minutes) instead of reconnecting on every pass. A failed
  link is only reported after three consecutive attempts, so a slow start no
  longer raises an error you need to dismiss.

  One consequence worth knowing: fonts for a document open at launch are now
  activated a short time after Affinity starts rather than immediately.

  Reported by zeusextremachine on Windows 11 with Affinity 3.3.1. (#16)

- **The Affinity link is far less demanding in general.** Reviewing the above
  turned up three further problems in the same code:

  - Every poll reopened a session from scratch, retrying all three loopback
    spellings and up to three protocol versions each time. The working address
    and protocol are now remembered after the first success and tried first,
    so a healthy link is one connection rather than up to nine attempts. Both
    are forgotten again on failure, so a changed setup is still found.
  - Documents were re-queried every 10 seconds, each time with a complete
    handshake. That is now every 30 seconds.
  - A server that accepted the connection but did not report its own version
    string was treated as having refused every protocol version, and the link
    failed for no reason. Acceptance and the reported version are now tracked
    separately.

## [0.7.0] — 2026-10-12

### Added

- **Copy a font's name with one click.** Click the family name in the detail
  panel to copy it, or the PostScript name in the metadata list to copy that
  instead. Useful when an application has no font picker and expects you to
  type the name into a config file by hand. (#14)

- **System tray.** ZFontManager can now live in the tray. Three new settings
  under *System*:

  - **Close to tray** — the close button hides the window instead of quitting.
  - **Start with system** — launch at sign-in.
  - **Start minimised** — start in the tray without opening the window.

  All three are off by default, so nothing changes unless you turn them on.

  On Windows, left-click the tray icon to bring the window back and right-click
  for the menu. On macOS and Linux the menu opens on click, following each
  platform's convention — Linux desktops do not deliver tray click events to
  the application at all, so the menu is the way back. If the tray cannot be
  created, *Close to tray* stays inactive rather than leaving you with no way
  to reach the window. (#15)

### Changed

- Tauri updated from 2.11.5 to 2.12.3, which the autostart integration
  requires. This also moves the underlying webview layer (wry) from 0.55 to
  0.57.

## [0.6.1] — 2026-09-22

### Added

- **Show only your own fonts.** A new *Show* filter at the top of the filter
  menu switches between all fonts, system fonts, and the fonts you installed
  yourself. On a machine with several hundred system faces, finding the ones
  you added by hand no longer means scrolling past everything the OS shipped.
  It composes with the other filters, so you can narrow to your own fonts and
  then filter those by format, feature or character.

  Requested by a user who found their own fonts hard to pick out of the list.

## [0.6.0] — 2026-09-21

Everything here comes from one report by a tester running Fedora on a 1366x768
ThinkPad. 

### Added

- **Interface scale.** ZFontManager now measures the usable area of your screen
  at startup and picks a scale that fits the whole interface into it, so a
  small or low-resolution display shows the entire layout instead of a cropped
  one. Settings → Appearance → *Interface scale* overrides it: **Auto**, or a
  fixed 80–125% if you would rather decide, which also serves anyone on a large
  display who simply wants the interface bigger.
- **Reduce transparency.** A new Appearance setting that replaces the frosted
  glass with solid panels. It is easier to read, it is the accessible choice,
  and it removes the most expensive thing the app draws — which makes a real
  difference on older integrated graphics.
- **Filter by file format.** OpenType, TrueType, WOFF and WOFF2, in the filter
  menu alongside classification and language support.
- **Filter by typographic feature.** Swashes, stylistic alternates,
  discretionary ligatures, small caps, oldstyle figures and fractions. The
  feature list is now read while the library is indexed rather than only when a
  font is selected, so it can be filtered on. If you are hunting for faces with
  alternate letterforms, this is the fastest way to find them.
- **Find fonts containing a specific character.** Type any character into the
  filter menu and the library narrows to the fonts whose character map covers
  it. The search reads every font file, so it runs on demand rather than
  continuously, and it reports the codepoint it resolved your character to.
- **Glyph count.** Shown in the inspector, and available as a sort order.
- **Collapsible sidebar sections.** Browse, Collections and Tags each fold away,
  and stay folded between launches.
- **Sort by glyph count**, next to name, styles and size.

### Fixed

- **The window could open taller than the screen.** The default size is right
  for the displays the app was designed against, but nothing checked whether it
  fitted. On a 1366x768 screen roughly a hundred pixels of the app sat below
  the bottom edge once the desktop panel was counted, which among other things
  hid the inspector's Tags section. The window is now measured against the
  monitor's usable area — panels, docks and taskbars excluded — and clamped and
  centred to fit. The configured default is unchanged, so nothing moves on a
  display that was already big enough.
- **The context menu could not be scrolled.** It had no height limit, so with
  enough tags the "Tag all" menu grew past the screen and lost its top. It now
  scrolls, and can no longer be positioned off the top or left edge.
- **Panels turned unreadable when the compositor could not blur.** Every
  overlay — the context menu, Compare, Settings, the selection bar — was a
  ten-percent-white surface that relied entirely on a backdrop blur to become
  legible. Where that blur silently does nothing, the panel was a haze. The new
  *Reduce transparency* setting makes them solid.
- **No resize handles.** ZFontManager draws its own title bar, and drawing your
  own means the window has no resize border unless you provide one. There was
  none, so the edges were a one-pixel guessing game. All four edges and corners
  are now grabbable, with the right cursor on each. This affected Windows and
  macOS too.
- **The online font browser ignored the preview size slider**, rendering every
  preview at a fixed size while the rest of the library followed the control.

### Notes

- The tiny flickering squares reported on Intel HD 4000 with the crocus driver
  are not drawn by the app and were not reproducible away from that hardware.
  *Reduce transparency* removes the most likely trigger; if you see them, please
  try it and say whether it helps.

## [0.5.1] — 2026-09-19

### Fixed

- **The sidebar now scrolls.** It had no scroll region at all, so once Browse,
  Collections and Tags together grew taller than the window, everything past
  the bottom edge was silently cut off — the Tags section first, then Trash,
  About and Shortcuts. 0.4.0 made this easy to hit by adding four rows to
  Browse. Browse, Collections and Tags now scroll; Trash, About and Shortcuts
  stay pinned at the bottom. Affected every platform.
- The onboarding tour scrolls its target into view before drawing the
  spotlight, so a step can no longer point at something below the window edge.

## [0.5.0] — 2026-09-17

### Added

- **Online font browser.** Search nearly two thousand open-licence families —
  the whole Google Fonts catalogue — and add any of them to the library with
  one click, from a new *Online* section in the sidebar. Filter by category,
  licence and variable-only. Each family shows its designer, licence, a link to
  the full licence text and its Reserved Font Name before you download; the
  licence text is stored alongside the font. Downloaded families are tagged
  *Online* automatically and marked *In library* in the browser afterwards.

  This is the first feature that contacts a real server, so the rules are
  strict and enforced in one place. Off by default. Switching it on sends
  nothing — a request goes out only when you open the section and search or
  download; there is no background refresh and no probe on startup. The
  catalogue is fetched once from `api.fontsource.org` and cached for seven
  days; search runs locally against that cache, debounced, so the provider
  sees one request a week rather than one per keystroke. Font files and
  licences come from the `google/fonts` repository via
  `raw.githubusercontent.com` — the canonical source, and the only one that
  serves complete fonts rather than per-script web subsets. Both hosts are
  named under the toggle. Every request passes through a single client that
  refuses if the setting is off, rejects any host not on that two-entry list,
  follows no redirects, and identifies itself as ZFontManager.

  The licence shown is read from the repository directory the files are
  actually fetched from, so it is by construction the licence that ships beside
  the font; the catalogue's licence field only decides which directory to try
  first.

  Opening a family shows a live preview of your current sample text in that
  font. One face — the Regular — is fetched for the preview through the same
  gated client, and if you then add the family the download reuses that file
  rather than fetching it again. Closing the family without adding it removes
  the preview file.

  Bunny Fonts and Fontsource's own CDN were evaluated and rejected as download
  sources: both serve only WOFF/WOFF2 or per-script subsets, which Windows
  cannot install and which would put a dozen partial copies of each style in
  the OS. Fontshare was rejected because the ITF Free Font Licence forbids
  redistribution.

- **System fonts now say why they can't be turned off.** On Windows and macOS
  the operating system owns its own fonts, and until now a system font showed
  a dimmed activation toggle that looked switched on but did nothing. It now
  shows a small lock badge reading *OS*, with the full explanation on hover:
  managed by the operating system, can't be turned off. On Linux, where
  fontconfig can exclude any font, the toggle is unchanged.
- **Hide them entirely.** The filter menu gains *Hide fonts the OS won't let
  you turn off*, so the library can show only what you can actually act on.
  It counts toward the filter badge and clears with the other filters.

### Changed

- `reqwest` now carries a TLS backend (`rustls`, using the OS trust store). It
  was built without one, which was fine for the Affinity integration's loopback
  HTTP but would have failed on any `https://` request.

## [0.4.0] — 2026-09-16

### Added

- **Import modes.** Dropping fonts in now offers *Add to library* (the file
  stays where it is and is registered from there) or *Copy to system folder*
  (copied into the library folder, original removed). On Linux and macOS a
  linked font is linked into the font folder so the rest of the system can see
  it — the original is never moved or deleted.
- **Custom library folder** under Settings, for deciding where copied imports
  are stored.
- **Auto-activate for Affinity by Canva** (Windows and macOS). While Affinity
  runs, fonts its open documents ask for are switched on, and switched back
  off when it quits. Needs Affinity 3.2+ with *Model Context Protocol* enabled.
  Off by default. Talks only to `localhost:6767` — nothing leaves the machine.
- **Auto-activate imported fonts**, an opt-in Settings toggle for batches
  under 64 fonts.
- Sidebar filters for **Activated**, **Activated until close**, **Deactivated**,
  **Last imported** and **System fonts**, each reachable from the command
  palette.
- Duplicate detection on import: fonts already in the library are reported and
  skipped instead of being added twice.
- Per-file actions in the name-conflict panel.
- **Select every font in view** with <kbd>Ctrl</kbd>/<kbd>⌘</kbd> + <kbd>A</kbd>,
  or *Select all* in the command palette, so a whole library — or whatever the
  current filters and search have narrowed it to — can be activated or
  deactivated in one action, with undo.

### Fixed

- Imported fonts were reported to the interface as inactive even when
  *auto-activate imports* was on and they had in fact been activated, so the
  library showed the wrong state until the next rescan.
- The Affinity watcher enumerated every running process every two seconds even
  when the feature was switched off, which was the default. It now does that
  only while the feature is enabled, and never starts at all on Linux, where
  Affinity does not exist.
- Deactivating a font that had been added with *Add to library* moved the
  original file out of its folder on macOS. Linked fonts are now linked and
  unlinked instead, leaving the original untouched.
- Fonts added with *Add to library* were invisible to other applications on
  Linux and macOS, because they sat outside the directories those systems
  scan.
- Removing a font added with *Add to library* only unregistered it on Windows.
  On Linux and macOS the font stayed available to other applications after it
  had been removed from the library.
- Opening Settings probed for Affinity even when Affinity auto-activation was
  switched off. Nothing is contacted now unless the feature is on, and the
  panel says *Disabled* instead of reporting it as offline.
- The source badge in the name-conflict panel showed *Managed* and *User* in
  English in every language.

### Changed

- The Privacy section of the README now lists, in a table, every feature that
  can open a connection, what it contacts and whether anything leaves your
  machine. Today that is one row, it is off by default, and it is loopback.
- **`CONTRIBUTING.md`** now sets out what a contribution has to satisfy: it must
  work on Linux, Windows and macOS; a genuinely platform-bound feature has to be
  declared, gated, hidden on platforms it cannot serve, and written up in the
  README; and any new interface text has to land in all nine languages with the
  plural forms each one requires.
- README documents the new import modes, the Affinity integration, the
  state filters in the sidebar and the name-conflict panel, and the Privacy
  section now states the local-loopback exception instead of claiming no
  network activity at all.
- All eight translations are complete again: German, Spanish, French,
  Japanese, Portuguese (Brazil), Russian, Simplified and Traditional Chinese
  each cover every string, with the plural forms their language requires.

## [0.3.0] — 2026-09-11

### Added

- **Apply a font in Photoshop or Illustrator** (Windows and macOS). Right-click a
  family → *Apply in Photoshop…* / *Apply in Illustrator…*, or use the command
  palette with a family selected. ZFontManager talks to the *running* app
  (COM on Windows, AppleScript on macOS) and switches every selected text
  layer or text frame to the font; with nothing selected it adds a new text
  layer using the family name as sample text. A sleeping font is woken "until
  close" first, so auditioning never installs anything permanently. Every
  outcome is reported in a toast (applied with layer count, layer created,
  app not running, no open document, app hasn't refreshed its font list yet).
  **Not yet verified against a real Photoshop/Illustrator** — the app-side
  plumbing and scripts are tested up to the Adobe boundary only. Feedback
  welcome.
- Adding or removing a watched folder now indexes the library immediately,
  shows the progress inline in Settings, and reports what changed
  ("3 new fonts found", "4 fonts removed", or that no font files were found).
- Linux bundles (AppImage, .deb, .rpm) are now built by the GitHub Actions
  workflow alongside the Windows and macOS installers, and
  `prepare-release.mjs` refreshes the website download page and checksums.

### Fixed

- **Watched folders on Windows.** Fonts in a folder added in Settings were
  shown as active, could not be deactivated ("font not found in per-user
  registry"), and every card said "This format can't be previewed yet".
  - Activation is now driven by what Windows itself lists in the font
    registry (read through the Win32 API instead of scraping `reg.exe`
    output). Fonts from a watched folder start *inactive*; switching one on
    registers it for your user account right where it is — external drives
    included — and switching it off unregisters it, without errors when there
    was nothing to unregister. Original registry names are remembered so a
    round trip leaves the registry exactly as it was.
  - Fonts registered from outside the per-user font folder are loaded again
    at app start so they are available to other apps immediately.
  - Bulk activation keeps going past a failing file and always persists what
    was applied.
- **Previews for fonts outside the standard folders.** Watched folders (and
  on Linux `/usr/local/share/fonts`) were never allowed through the preview
  asset scope, so their fonts could not render. Every scanned folder is now
  allowed at startup and whenever the folder list changes.
- **Constant GPU load.** A perpetual background-gradient animation kept the
  compositor busy on every frame and forced the blurred sidebar and detail
  panel to re-blur continuously — up to ~50 % GPU on large displays and
  integrated GPUs. The gradient is now static; idle GPU use drops to zero.
- The in-app "Reduce motion" setting now also disables CSS animations
  (skeleton shimmer, toast flashes), not only the interface springs.
- macOS: a deactivated (parked) font lost its preview, glyph map and
  OpenType feature list because those were read from the original path.
- Registry entries ZFontManager writes on Windows now use the names Windows
  itself would use: `(OpenType)` for CFF files, every face named for
  collections, and a numbered suffix when a name is already taken.

### Changed

- All nine languages gained the strings for the features above.

## [0.2.0] and earlier

Earlier releases predate this changelog. Their installers remain available on
the [download page](https://zsync.eu/zfontmanager/).

[0.3.0]: https://github.com/TheHolyOneZ/ZFontManager/releases/tag/v0.3.0
[0.5.1]: https://github.com/TheHolyOneZ/ZFontManager/releases/tag/v0.5.1
[0.6.0]: https://github.com/TheHolyOneZ/ZFontManager/releases/tag/v0.6.0
