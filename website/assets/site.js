// Jellyfesh website: language switching and the download list.
// No cookies, no analytics, no third-party code. The chosen language is kept
// in localStorage when the browser allows it.

const STRINGS = {
  en: {
    "nav.home": "Home", "nav.download": "Download", "nav.privacy": "Privacy",
    "hero.title": "An elegant, very light music player",
    "hero.sub": "Your music, local and online, with lyrics that follow every word. For macOS, Windows, Linux, Android and the web.",
    "hero.download": "Download", "hero.notes": "What’s new",
    "hero.web": "Open Jellyfesh Web", "hero.downloadFor": "Download for {os}", "hero.sample": "Jellyfesh Web with sample music.",
    "features.title": "Made to be listened to",
    "features.lead": "One small Rust core does the work; each system gets a native app on top.",
    "f1.t": "Every word, on time", "f1.d": "Word-by-word lyrics with translation and romanization, drawn in step with what you hear.",
    "f2.t": "Albums as one piece", "f2.d": "Gapless playback, crossfade, ReplayGain, a 10-band equaliser and a limiter that only steps in when needed.",
    "f3.t": "Your servers, sandboxed", "f3.d": "Online sources are plugins in a strict sandbox. Subsonic, Navidrome and Jellyfin come first.",
    "f4.t": "Light on your machine", "f4.d": "Native apps over one Rust core. When nothing moves on screen, nothing is drawn.",
    "f5.t": "Colour from the cover", "f5.d": "The theme follows the artwork, and text stays readable on every pixel.",
    "f6.t": "Private by design", "f6.d": "No account and no tracking. Your library stays on your device.",
    "shots.title": "A first look", "shots.lead": "The Linux app in this release. The macOS, Windows and Android apps share its design.",
    "footer.rights": "© 2026 Jellyfesh. All rights reserved.", "footer.notices": "Third-party notices", "footer.source": "Source",
    "dl.title": "Download Jellyfesh",
    "dl.lead": "Version {version}. Builds are not signed yet; the note under each system says how to open them.",
    "dl.forYou": "For this device", "dl.soon": "Coming soon",
    "dl.soonNote": "Built automatically once the project’s repository is published.",
    "dl.verify": "Check a download with sha256sum (Linux), shasum -a 256 (macOS) or Get-FileHash (Windows) against the SHA-256 shown.",
    "dl.cli": "Command line", "dl.cli.req": "The jf player for terminals and scripts",
    "dl.plugins": "Official plugins", "dl.plugins.req": "Subsonic, OpenSubsonic (Navidrome and others) and Jellyfin",
    "dl.plugins.note": "Every app already includes them. Download one to update it between releases: Settings → Sources → Install plugin.",
    "dl.web": "Web", "dl.web.req": "In your browser: a current Chrome, Edge, Firefox or Safari · for your Subsonic, Navidrome or Jellyfin server",
    "dl.web.note": "Nothing to install; add it to your home screen or dock if you like. The zip is the same app, to host next to your own server.",
    "dl.unavailable": "The download list could not be loaded. Open this page from the website, not from a file.",
    "os.macos": "macOS", "os.macos.req": "macOS 14.2 or later · Apple silicon and Intel",
    "os.macos.note": "Unsigned for now: the first time, right-click the app and choose Open.",
    "os.windows": "Windows", "os.windows.req": "Windows 10 (1809) or later · x64 and Arm64",
    "os.windows.note": "Unsigned for now: if SmartScreen appears, choose More info, then Run anyway.",
    "os.linux": "Linux", "os.linux.req": "GTK 4.14 and libadwaita 1.5 or later · x86_64",
    "os.linux.note": "Unpack the archive and run ./install.sh, or install the Flatpak bundle.",
    "os.android": "Android", "os.android.req": "Android 8.0 or later",
    "os.android.note": "Allow installing apps from this source when Android asks.",
    "privacy.title": "Privacy",
    "privacy.p1": "Jellyfesh has no account, no analytics and no tracking. It sends nothing about you or your library anywhere.",
    "privacy.p2": "Your library, playlists, settings and play history are stored on your device.",
    "privacy.p3": "Online sources are plugins you choose to install. A plugin contacts only the servers its manifest lists, such as your own Subsonic or Jellyfin server, and only for what you ask it to do. Sign-in tokens are kept in your system’s keychain; passwords are never stored.",
    "privacy.p4": "Community plugins are off by default. Before you enable one, Jellyfesh explains that it is unofficial and may break the service’s terms.",
    "privacy.p5": "This website uses no cookies, no analytics and no third-party scripts or fonts.",
  },
  "zh-Hans": {
    "nav.home": "首页", "nav.download": "下载", "nav.privacy": "隐私",
    "hero.title": "优雅、轻盈的音乐播放器",
    "hero.sub": "本地与在线音乐，歌词逐字跟随。支持 macOS、Windows、Linux、Android 和网页。",
    "hero.download": "下载", "hero.notes": "更新内容",
    "hero.web": "打开 Jellyfesh Web", "hero.downloadFor": "下载 {os} 版", "hero.sample": "Jellyfesh Web，播放示例音乐。",
    "features.title": "为聆听而生",
    "features.lead": "由一个小巧的 Rust 内核完成工作，每个系统上都是原生应用。",
    "f1.t": "每个字，都准时", "f1.d": "逐字歌词，含翻译与罗马音，与你听到的声音同步呈现。",
    "f2.t": "专辑浑然一体", "f2.d": "无缝播放、淡入淡出、ReplayGain、10 段均衡器，以及仅在需要时介入的限幅器。",
    "f3.t": "你的服务器，沙盒运行", "f3.d": "在线来源以插件形式运行在严格的沙盒中，率先支持 Subsonic、Navidrome 和 Jellyfin。",
    "f4.t": "轻装上阵", "f4.d": "原生应用，共享同一个 Rust 内核。画面静止时，不做任何绘制。",
    "f5.t": "色彩取自封面", "f5.d": "主题随封面变化，文字在每个像素上都清晰可读。",
    "f6.t": "隐私为本", "f6.d": "无需账号，没有追踪。你的音乐库只留在你的设备上。",
    "shots.title": "抢先一览", "shots.lead": "本版本的 Linux 应用。macOS、Windows 和 Android 应用采用同样的设计。",
    "footer.rights": "© 2026 Jellyfesh。保留所有权利。", "footer.notices": "第三方声明", "footer.source": "源代码",
    "dl.title": "下载 Jellyfesh",
    "dl.lead": "版本 {version}。安装包尚未签名，各系统下方的说明介绍了打开方法。",
    "dl.forYou": "适用于本设备", "dl.soon": "即将推出",
    "dl.soonNote": "项目仓库发布后将自动构建。",
    "dl.verify": "可用 sha256sum（Linux）、shasum -a 256（macOS）或 Get-FileHash（Windows）核对下载文件与所示 SHA-256 是否一致。",
    "dl.cli": "命令行", "dl.cli.req": "适用于终端和脚本的 jf 播放器",
    "dl.plugins": "官方插件", "dl.plugins.req": "Subsonic、OpenSubsonic（Navidrome 等）和 Jellyfin",
    "dl.plugins.note": "每个应用都已内置。如需在两次发布之间更新，请下载后在“设置 → 来源 → 安装插件”中安装。",
    "dl.web": "网页版", "dl.web.req": "在浏览器中使用：新版 Chrome、Edge、Firefox 或 Safari · 适用于你的 Subsonic、Navidrome 或 Jellyfin 服务器",
    "dl.web.note": "无需安装；也可以添加到主屏幕或程序坞。zip 文件是同一个应用，可以放在你自己的服务器旁托管。",
    "dl.unavailable": "无法载入下载列表。请从网站打开此页面，而不是从本地文件打开。",
    "os.macos": "macOS", "os.macos.req": "macOS 14.2 或更高版本 · Apple 芯片与 Intel",
    "os.macos.note": "暂未签名：首次打开时，请右键点击应用并选择“打开”。",
    "os.windows": "Windows", "os.windows.req": "Windows 10（1809）或更高版本 · x64 与 Arm64",
    "os.windows.note": "暂未签名：如出现 SmartScreen，请点击“更多信息”，再点击“仍要运行”。",
    "os.linux": "Linux", "os.linux.req": "GTK 4.14 与 libadwaita 1.5 或更高版本 · x86_64",
    "os.linux.note": "解压后运行 ./install.sh，或安装 Flatpak 包。",
    "os.android": "Android", "os.android.req": "Android 8.0 或更高版本",
    "os.android.note": "Android 询问时，请允许安装来自此来源的应用。",
    "privacy.title": "隐私",
    "privacy.p1": "Jellyfesh 无需账号，没有统计分析，也没有追踪。它不会向任何地方发送关于你或你的音乐库的信息。",
    "privacy.p2": "你的音乐库、播放列表、设置和播放记录都保存在你的设备上。",
    "privacy.p3": "在线来源是你自行选择安装的插件。插件只会连接其清单中列出的服务器（例如你自己的 Subsonic 或 Jellyfin 服务器），且只执行你要求的操作。登录令牌保存在系统钥匙串中，密码从不保存。",
    "privacy.p4": "社区插件默认关闭。启用前，Jellyfesh 会说明它属于非官方插件，并可能违反相应服务的条款。",
    "privacy.p5": "本网站不使用 Cookie、统计分析，也不加载任何第三方脚本或字体。",
  },
  "zh-Hant": {
    "nav.home": "首頁", "nav.download": "下載", "nav.privacy": "隱私",
    "hero.title": "優雅、輕盈的音樂播放器",
    "hero.sub": "本機與線上音樂，歌詞逐字跟隨。支援 macOS、Windows、Linux、Android 與網頁。",
    "hero.download": "下載", "hero.notes": "更新內容",
    "hero.web": "開啟 Jellyfesh Web", "hero.downloadFor": "下載 {os} 版", "hero.sample": "Jellyfesh Web，播放範例音樂。",
    "features.title": "為聆聽而生",
    "features.lead": "由一個小巧的 Rust 核心完成工作，每個系統上都是原生應用程式。",
    "f1.t": "每個字，都準時", "f1.d": "逐字歌詞，含翻譯與羅馬拼音，與你聽到的聲音同步呈現。",
    "f2.t": "專輯渾然一體", "f2.d": "無縫播放、淡入淡出、ReplayGain、10 段等化器，以及僅在需要時介入的限幅器。",
    "f3.t": "你的伺服器，沙盒執行", "f3.d": "線上來源以外掛形式在嚴格的沙盒中執行，率先支援 Subsonic、Navidrome 與 Jellyfin。",
    "f4.t": "輕裝上陣", "f4.d": "原生應用程式，共用同一個 Rust 核心。畫面靜止時，不做任何繪製。",
    "f5.t": "色彩取自封面", "f5.d": "主題隨封面變化，文字在每個像素上都清晰可讀。",
    "f6.t": "隱私為本", "f6.d": "無需帳號，沒有追蹤。你的音樂庫只留在你的裝置上。",
    "shots.title": "搶先一覽", "shots.lead": "本版本的 Linux 應用程式。macOS、Windows 與 Android 應用程式採用相同的設計。",
    "footer.rights": "© 2026 Jellyfesh。保留所有權利。", "footer.notices": "第三方聲明", "footer.source": "原始碼",
    "dl.title": "下載 Jellyfesh",
    "dl.lead": "版本 {version}。安裝檔尚未簽署，各系統下方的說明介紹了開啟方式。",
    "dl.forYou": "適用於本裝置", "dl.soon": "即將推出",
    "dl.soonNote": "專案儲存庫發布後將自動建置。",
    "dl.verify": "可用 sha256sum（Linux）、shasum -a 256（macOS）或 Get-FileHash（Windows）核對下載檔案與所示 SHA-256 是否相符。",
    "dl.cli": "命令列", "dl.cli.req": "適用於終端機與指令碼的 jf 播放器",
    "dl.plugins": "官方外掛", "dl.plugins.req": "Subsonic、OpenSubsonic（Navidrome 等）與 Jellyfin",
    "dl.plugins.note": "每個應用程式皆已內建。如需在兩次發行之間更新，請下載後在「設定 → 來源 → 安裝外掛」中安裝。",
    "dl.web": "網頁版", "dl.web.req": "在瀏覽器中使用：新版 Chrome、Edge、Firefox 或 Safari · 適用於你的 Subsonic、Navidrome 或 Jellyfin 伺服器",
    "dl.web.note": "無需安裝；也可以加入主畫面或 Dock。zip 檔是同一個應用程式，可放在你自己的伺服器旁架設。",
    "dl.unavailable": "無法載入下載清單。請從網站開啟此頁面，而不是從本機檔案開啟。",
    "os.macos": "macOS", "os.macos.req": "macOS 14.2 或更新版本 · Apple 晶片與 Intel",
    "os.macos.note": "暫未簽署：首次開啟時，請在 App 上按右鍵並選擇「打開」。",
    "os.windows": "Windows", "os.windows.req": "Windows 10（1809）或更新版本 · x64 與 Arm64",
    "os.windows.note": "暫未簽署：如出現 SmartScreen，請按「其他資訊」，再按「仍要執行」。",
    "os.linux": "Linux", "os.linux.req": "GTK 4.14 與 libadwaita 1.5 或更新版本 · x86_64",
    "os.linux.note": "解壓縮後執行 ./install.sh，或安裝 Flatpak 套件。",
    "os.android": "Android", "os.android.req": "Android 8.0 或更新版本",
    "os.android.note": "Android 詢問時，請允許安裝來自此來源的應用程式。",
    "privacy.title": "隱私",
    "privacy.p1": "Jellyfesh 無需帳號，沒有統計分析，也沒有追蹤。它不會向任何地方傳送關於你或你的音樂庫的資訊。",
    "privacy.p2": "你的音樂庫、播放清單、設定與播放紀錄都儲存在你的裝置上。",
    "privacy.p3": "線上來源是你自行選擇安裝的外掛。外掛只會連線至其資訊清單中列出的伺服器（例如你自己的 Subsonic 或 Jellyfin 伺服器），且只執行你要求的操作。登入權杖儲存在系統鑰匙圈中，密碼從不儲存。",
    "privacy.p4": "社群外掛預設關閉。啟用前，Jellyfesh 會說明它屬於非官方外掛，並可能違反相應服務的條款。",
    "privacy.p5": "本網站不使用 Cookie、統計分析，也不載入任何第三方指令碼或字型。",
  },
  ja: {
    "nav.home": "ホーム", "nav.download": "ダウンロード", "nav.privacy": "プライバシー",
    "hero.title": "上品で、とても軽い音楽プレーヤー",
    "hero.sub": "ローカルもオンラインも。歌詞は一語ずつ寄り添います。macOS、Windows、Linux、Android、Web に対応。",
    "hero.download": "ダウンロード", "hero.notes": "新機能",
    "hero.web": "Jellyfesh Web を開く", "hero.downloadFor": "{os} 版をダウンロード", "hero.sample": "サンプル音楽を再生中の Jellyfesh Web。",
    "features.title": "聴くためにつくられました",
    "features.lead": "小さな Rust コアがすべてを担い、各システムにはネイティブのアプリを。",
    "f1.t": "一語一語、ぴったりに", "f1.d": "翻訳とローマ字付きの逐語歌詞を、聴こえる音に合わせて表示します。",
    "f2.t": "アルバムをひとつながりで", "f2.d": "ギャップレス再生、クロスフェード、ReplayGain、10 バンドのイコライザー、必要なときだけ働くリミッター。",
    "f3.t": "あなたのサーバーを、サンドボックスで", "f3.d": "オンラインのソースは厳格なサンドボックス内のプラグイン。まずは Subsonic、Navidrome、Jellyfin に対応します。",
    "f4.t": "マシンにやさしく", "f4.d": "ひとつの Rust コアの上にネイティブアプリ。画面が動かないときは、何も描きません。",
    "f5.t": "色はジャケットから", "f5.d": "テーマはアートワークに合わせて変わり、文字はどこでも読みやすいまま。",
    "f6.t": "プライバシーを第一に", "f6.d": "アカウント不要、トラッキングなし。ライブラリはあなたのデバイスに。",
    "shots.title": "ひと足先に", "shots.lead": "このリリースの Linux アプリ。macOS、Windows、Android 版も同じデザインです。",
    "footer.rights": "© 2026 Jellyfesh. All rights reserved.", "footer.notices": "サードパーティのライセンス", "footer.source": "ソース",
    "dl.title": "Jellyfesh をダウンロード",
    "dl.lead": "バージョン {version}。ビルドはまだ署名されていません。各システムの下に開き方を記載しています。",
    "dl.forYou": "このデバイス向け", "dl.soon": "近日公開",
    "dl.soonNote": "プロジェクトのリポジトリ公開後に自動でビルドされます。",
    "dl.verify": "sha256sum（Linux）、shasum -a 256（macOS）、Get-FileHash（Windows）で、表示されている SHA-256 と照合できます。",
    "dl.cli": "コマンドライン", "dl.cli.req": "ターミナルやスクリプト向けの jf プレーヤー",
    "dl.plugins": "公式プラグイン", "dl.plugins.req": "Subsonic、OpenSubsonic（Navidrome など）、Jellyfin",
    "dl.plugins.note": "どのアプリにも最初から入っています。リリースの合間に更新するときは、ダウンロードして「設定 → ソース → プラグインをインストール」から入れてください。",
    "dl.web": "Web 版", "dl.web.req": "ブラウザで使えます：最新の Chrome・Edge・Firefox・Safari · Subsonic・Navidrome・Jellyfin サーバー向け",
    "dl.web.note": "インストールは不要です。ホーム画面や Dock に追加することもできます。zip は同じアプリで、自分のサーバーの隣でホストするためのものです。",
    "dl.unavailable": "ダウンロード一覧を読み込めませんでした。ファイルからではなく、ウェブサイトからこのページを開いてください。",
    "os.macos": "macOS", "os.macos.req": "macOS 14.2 以降 · Apple シリコンと Intel",
    "os.macos.note": "現在は未署名です。初回はアプリを右クリックして「開く」を選んでください。",
    "os.windows": "Windows", "os.windows.req": "Windows 10（1809）以降 · x64 と Arm64",
    "os.windows.note": "現在は未署名です。SmartScreen が表示されたら「詳細情報」→「実行」を選んでください。",
    "os.linux": "Linux", "os.linux.req": "GTK 4.14 と libadwaita 1.5 以降 · x86_64",
    "os.linux.note": "アーカイブを展開して ./install.sh を実行するか、Flatpak バンドルをインストールしてください。",
    "os.android": "Android", "os.android.req": "Android 8.0 以降",
    "os.android.note": "Android に確認されたら、このソースからのインストールを許可してください。",
    "privacy.title": "プライバシー",
    "privacy.p1": "Jellyfesh にはアカウントも、解析も、トラッキングもありません。あなたやライブラリに関する情報をどこにも送信しません。",
    "privacy.p2": "ライブラリ、プレイリスト、設定、再生履歴はあなたのデバイスに保存されます。",
    "privacy.p3": "オンラインのソースは、あなたが選んでインストールするプラグインです。プラグインはマニフェストに記載されたサーバー（あなたの Subsonic や Jellyfin サーバーなど）にだけ、頼まれたことのためにだけ接続します。サインインのトークンはシステムのキーチェーンに保管され、パスワードは保存されません。",
    "privacy.p4": "コミュニティ製プラグインは初期状態でオフです。有効にする前に、非公式であり、サービスの規約に反する可能性があることを説明します。",
    "privacy.p5": "このウェブサイトは Cookie、解析、サードパーティのスクリプトやフォントを使用しません。",
  },
};

const LANGS = [["en", "English"], ["zh-Hans", "简体中文"], ["zh-Hant", "繁體中文"], ["ja", "日本語"]];

function pickLanguage() {
  try {
    const saved = localStorage.getItem("lang");
    if (saved && STRINGS[saved]) return saved;
  } catch (_) { /* storage may be blocked */ }
  for (const l of navigator.languages || [navigator.language || "en"]) {
    const tag = l.toLowerCase();
    if (tag.startsWith("zh")) {
      return /hant|tw|hk|mo/.test(tag) ? "zh-Hant" : "zh-Hans";
    }
    if (tag.startsWith("ja")) return "ja";
    if (tag.startsWith("en")) return "en";
  }
  return "en";
}

let lang = pickLanguage();
const t = (key, vars = {}) =>
  (STRINGS[lang][key] ?? STRINGS.en[key] ?? key).replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? "");

function applyLanguage() {
  document.documentElement.lang = lang;
  for (const el of document.querySelectorAll("[data-i18n]")) {
    el.textContent = t(el.dataset.i18n, window.__vars || {});
  }
  // The hero's Download names the visitor's system when it is one Jellyfesh is made for.
  const os = detectOs();
  for (const el of document.querySelectorAll("[data-download-for]")) {
    if (os) el.textContent = t("hero.downloadFor", { os: t(`os.${os}`) });
  }
  const select = document.querySelector(".lang");
  if (select) select.value = lang;
}

function setupLanguagePicker() {
  const select = document.querySelector(".lang");
  if (!select) return;
  for (const [code, name] of LANGS) {
    const opt = document.createElement("option");
    opt.value = code;
    opt.textContent = name;
    select.append(opt);
  }
  select.addEventListener("change", () => {
    lang = select.value;
    try { localStorage.setItem("lang", lang); } catch (_) { /* fine */ }
    applyLanguage();
    if (window.__renderDownloads) window.__renderDownloads();
  });
}

function detectOs() {
  const ua = navigator.userAgent;
  if (/Android/i.test(ua)) return "android";
  if (/Mac/i.test(ua)) return "macos";
  if (/Win/i.test(ua)) return "windows";
  if (/Linux/i.test(ua)) return "linux";
  return null;
}

function formatSize(bytes) {
  if (!bytes) return "";
  const mb = bytes / (1024 * 1024);
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

async function setupDownloads() {
  const host = document.getElementById("downloads");
  if (!host) return;
  let manifest;
  try {
    const response = await fetch("releases.json", { cache: "no-cache" });
    manifest = await response.json();
  } catch (_) {
    host.innerHTML = "";
    const p = document.createElement("p");
    p.className = "note";
    p.dataset.i18n = "dl.unavailable";
    host.append(p);
    applyLanguage();
    return;
  }
  window.__vars = { version: manifest.version };
  const mine = detectOs();
  const order = ["web", "macos", "windows", "linux", "android", "plugins", "cli"];
  // Jellyfesh Web is at app/ when the site was built with it (build_site.py --app).
  const webApp = manifest.web_app || null;
  window.__renderDownloads = () => {
    host.innerHTML = "";
    for (const platform of order) {
      const files = manifest.files.filter((f) => f.platform === platform);
      if (platform === "web" && !webApp && files.length === 0) continue;
      const card = document.createElement("article");
      card.className = "card platform" + (platform === mine ? " recommended" : "");
      const h3 = document.createElement("h3");
      const title = document.createElement("span");
      const own = platform === "cli" || platform === "plugins" || platform === "web";
      title.textContent = own ? t(`dl.${platform}`) : t(`os.${platform}`);
      h3.append(title);
      if (platform === mine) {
        const b = document.createElement("span");
        b.className = "badge";
        b.textContent = t("dl.forYou");
        h3.append(b);
      } else if (files.length === 0 && platform !== "web") {
        const b = document.createElement("span");
        b.className = "badge soon";
        b.textContent = t("dl.soon");
        h3.append(b);
      }
      card.append(h3);
      const req = document.createElement("p");
      req.className = "req";
      req.textContent = own ? t(`dl.${platform}.req`) : t(`os.${platform}.req`);
      card.append(req);
      if (platform === "web" && webApp) {
        const open = document.createElement("a");
        open.className = "button solid";
        open.href = webApp;
        open.textContent = t("hero.web");
        card.append(open);
      }
      if (files.length) {
        const ul = document.createElement("ul");
        for (const f of files) {
          const li = document.createElement("li");
          const a = document.createElement("a");
          a.className = "file";
          a.href = f.url;
          a.download = "";
          const name = document.createElement("span");
          name.textContent = f.label || f.name;
          const size = document.createElement("span");
          size.className = "size";
          size.textContent = formatSize(f.size);
          a.append(name, size);
          const sha = document.createElement("span");
          sha.className = "sha";
          sha.textContent = `SHA-256 ${f.sha256}`;
          li.append(a, sha);
          ul.append(li);
        }
        card.append(ul);
        if (platform !== "cli") {
          const note = document.createElement("p");
          note.className = "note";
          note.textContent = own ? t(`dl.${platform}.note`) : t(`os.${platform}.note`);
          card.append(note);
        }
      } else if (platform === "web") {
        const note = document.createElement("p");
        note.className = "note";
        note.textContent = t("dl.web.note");
        card.append(note);
      } else {
        const note = document.createElement("p");
        note.className = "note";
        note.textContent = t("dl.soonNote");
        card.append(note);
      }
      host.append(card);
    }
    applyLanguage();
  };
  window.__renderDownloads();
}

async function setupWebLink() {
  const link = document.querySelector("[data-web-app]");
  if (!link) return;
  try {
    const manifest = await (await fetch("releases.json", { cache: "no-cache" })).json();
    if (manifest.web_app) {
      link.href = manifest.web_app;
      link.hidden = false;
    }
  } catch (_) { /* opened from a file: no link */ }
}

// The picture of the app is copied in when the site is built (tools/release/build_site.py); a
// copy of website/ opened as files has none, so the figure goes rather than show a broken image.
function setupAppShot() {
  const figure = document.querySelector("[data-app-shot]");
  const img = figure && figure.querySelector("img");
  if (!img) return;
  const hide = () => { figure.hidden = true; };
  if (img.complete && img.naturalWidth === 0) hide();
  else img.addEventListener("error", hide);
}

setupLanguagePicker();
applyLanguage();
setupDownloads();
setupWebLink();
setupAppShot();
