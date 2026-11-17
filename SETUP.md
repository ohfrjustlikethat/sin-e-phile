# Setting up sin·e·phile

Everything the app needs or can use, where to get each piece from its official source, and how
to add it to the app. **Most of it is optional**, and the app says so where it matters. This is
for using the app; building it from source is [`docs/SETUP.md`](docs/SETUP.md).

> **sin·e·phile comes with no content sources.** It names none and links to none. It plays your
> own files, public-domain films from the Internet Archive, and whatever add-ons **you** choose
> to add by their address. What you add is your responsibility.

---

## The essentials

### 1. Windows 10 or 11, with WebView2

The app's screens run in Microsoft's WebView2. Windows 10 and 11 usually have it already.

- **Get it:** [WebView2 Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
  (Microsoft).
- **Add it to the app:** nothing to do; the app finds it.

### 2. FFmpeg

Reads your files (length, tracks), lines subtitles up with the speech, and makes the scrub-bar
previews.

- **Get it:** in a terminal, `winget install Gyan.FFmpeg`, or the official builds linked from
  [ffmpeg.org/download.html](https://ffmpeg.org/download.html).
- **Add it to the app:** nothing to do, as long as `ffmpeg` and `ffprobe` are on your PATH
  (winget does this). Open a new terminal and run `ffmpeg -version` to check.

### 3. libmpv (the player)

The video player itself.

- **Get it:** a release build comes with it. From source, `python tools/mpv/fetch.py` fetches the
  pinned build from the
  [mpv-winbuild-cmake releases](https://github.com/shinchiro/mpv-winbuild-cmake/releases) and
  checks its checksum.
- **Add it to the app:** `libmpv-2.dll` beside `sin-e-phile.exe`, or in a `lib` folder beside it.
  To use one elsewhere, set the `SINEPHILE_LIBMPV` environment variable to its full path.

### 4. The catalogue

Every film and series the app knows about, from IMDb's free datasets.

- **Get it:** the app downloads and builds it itself on first launch, then **keeps it current on
  its own**: it checks every hour and adds new titles and episodes when IMDb publishes them,
  about once a day. Settings → Catalogue shows when it last changed, with **Refresh now**.
- **Licence:** IMDb's datasets are for personal, non-commercial use. Your copy stays on your PC.

---

## Making it yours

### 5. Your films and shows on this PC

- **Add them:** first run, step 3, or **Settings → Library → Add a folder…**. The app reads the
  folder at once. Shows are read from their folders:
  `Show (Year)/Season 1/Show - S01E01 - Title.mkv`. Featurettes, trailers and other extras are
  left out. Anything it cannot place for certain waits for you in **Check them**.

### 6. Sources: add-ons, by their address

An add-on lets the app find a film to stream or download. **The app suggests none.**

- **Add one:** first run, step 4, or **Settings → Sources → Add a source**. Paste the add-on's
  address (it ends in `manifest.json`). The app fetches it and shows you what it offers **before**
  you add it. Turn sources on or off, and put them in your order, in the same place.
- **Without one:** a film that is not on your PC (and not in the Internet Archive) says
  "Nothing you have set up can play or download this yet", with an **Add a source** button.

### 7. Subtitles: an OpenSubtitles key (optional)

For a film with no subtitles of its own in your languages.

- **Get it:** a free account at OpenSubtitles, then its API consumers page. In the app, **Get a free
  key** opens that page.
- **Add it to the app:** first run, step 5, or **Settings → Subtitles → OpenSubtitles**. It is
  checked as you save it; a refused key is not saved. Free accounts have a small daily download
  allowance, which Settings shows.
- **In the player:** **Menu** (≡ on a controller) → Subtitles: **Find subtitles**, **Load a
  file…**, **Align again**, timing in 0.1 s steps, and the style (size, position, colour, outline,
  box, font), kept for your profile.

### 8. What's showing now: a TMDB key (optional)

The catalogue knows each film's year; TMDB knows what is in cinemas and on the air this week.

- **Get it:** a free account at TMDB, then **Settings → API** on its site. In the app, **Get a free
  key** opens that page. Paste the **API Key** (32 characters).
- **Add it to the app:** first run, step 6, or **Settings → TMDB**. It is checked as you save it.
  Then Home's **New releases** starts with what is showing now, refreshed daily. Removing the key
  removes what it fetched.
- No key comes with the app; yours is kept on this PC, for your profile only.

### 9. Downloads: the built-in engine, or your own qBittorrent (optional)

- **Built in:** works with no setup. Downloads go to the `data` folder beside the app.
- **Your own qBittorrent:** if you already use it, **Downloads → Settings** (or first run, step 7):
  its Web UI address, user and password, and **Check**. The app uses it only to move files; it
  never reads its search plugins, feeds or tracker lists.
- **Get qBittorrent:** from its official website (search for "qBittorrent download").

### 10. A VPN (optional, recommended for torrents)

- **Add it to the app:** **Settings → Network**: choose your VPN's network adapter. Torrent
  traffic is then bound to it, and **stops if the VPN drops** rather than leaving by your ordinary
  connection.

### 11. An Xbox controller (optional)

- **Add it:** connect it (USB or Bluetooth). The whole app works with it: the D-pad and left stick
  move, **A** chooses, **B** goes back, the **right stick scrolls**. In the player, **↑ ↓** volume,
  **← →** seek, **Y** subtitles, **X** sound, **LB / RB** previous and next episode, **Menu** the
  full panel.

---

## Checking it all

From the source folder, `python tools/doctor/doctor.py` lists what is present and what is
missing, with how to fix each.
