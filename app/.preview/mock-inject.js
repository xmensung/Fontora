/* 浏览器预览用：伪造 Tauri IPC，让界面在没有后端的情况下也能渲染。 */
(function () {
  var F = function (file, family, style, extra) {
    var base = {
      id: file + "|0",
      path: "C:\\Fonts\\" + file,
      previewPath: "C:\\Fonts\\" + file,
      faceIndex: 0,
      family: family,
      style: style,
      postscriptName: family.replace(/\s+/g, "") + "-" + style,
      foundry: "Microsoft",
      license: "Microsoft",
      licenseUrl: null,
      format: /\.otf$/i.test(file) ? "otf" : "ttf",
      isVariable: false,
      axes: [],
      weight: 400,
      italic: /italic|oblique/i.test(style),
      monospaced: /mono|consol/i.test(file),
      classification: "sans",
      scripts: ["Latin", "Cyrillic"],
      features: ["kern", "liga"],
      glyphCount: 3200,
      fileSize: 240000,
      modified: 1700000000,
      source: "system",
      deactivatable: false,
      active: true,
    };
    for (var k in extra || {}) base[k] = extra[k];
    return base;
  };

  var fonts = [
    F("segoeui.ttf", "Segoe UI", "Regular", { classification: "sans", glyphCount: 2841 }),
    F("arial.ttf", "Arial", "Regular", { classification: "sans", glyphCount: 3418 }),
    F("georgia.ttf", "Georgia", "Regular", { classification: "serif", glyphCount: 2134 }),
    F("times.ttf", "Times New Roman", "Regular", { classification: "serif", glyphCount: 2728 }),
    F("consola.ttf", "Consolas", "Regular", { classification: "mono", glyphCount: 1094 }),
    F("impact.ttf", "Impact", "Regular", { classification: "display", glyphCount: 636 }),
    F("comic.ttf", "Comic Sans MS", "Regular", { classification: "script", glyphCount: 754 }),
    F("verdana.ttf", "Verdana", "Regular", { classification: "sans", glyphCount: 2133 }),
    F("arial.ttf", "Arial", "Bold", {
      id: "arial.ttf|1",
      faceIndex: 1,
      weight: 700,
      previewPath: "C:\\Fonts\\arialbd.ttf",
      path: "C:\\Fonts\\arialbd.ttf",
      classification: "sans",
    }),
    F("georgia.ttf", "Georgia", "Italic", {
      id: "georgia.ttf|2",
      faceIndex: 2,
      weight: 400,
      italic: true,
      previewPath: "C:\\Fonts\\georgiai.ttf",
      path: "C:\\Fonts\\georgiai.ttf",
      classification: "serif",
    }),
  ];

  var settings = {
    extraDirs: ["G:\\Mine\\素材类\\字体"],
    watchEnabled: true,
    autoActivateImports: false,
    libraryDir: "G:\\Mine\\素材类\\字体",
    libraryDirEnabled: true,
    affinityEnabled: true,
    affinityDeactivateOnQuit: true,
    onlineFontsEnabled: false,
  };

  var cbId = 1;
  var handlers = {};
  var query = new URLSearchParams(location.search);
  window.__MOCK_THEME__ = query.get("theme") || "light";
  window.__MOCK_OPEN_SETTINGS__ = query.get("settings") === "1";

  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: function (cb, once) {
      var id = cbId++;
      handlers[id] = { cb: cb, once: once };
      return id;
    },
    unregisterCallback: function (id) {
      delete handlers[id];
    },
    convertFileSrc: function (p) {
      var name = String(p).split(/[\\/]/).pop();
      var alias = {
        "arialbd.ttf": "arial.ttf",
        "georgiai.ttf": "georgia.ttf",
      };
      return "/mock-fonts/" + (alias[name] || name);
    },
    invoke: function (cmd, args) {
      switch (cmd) {
        case "plugin:event|listen":
          return Promise.resolve(1);
        case "plugin:event|unlisten":
          return Promise.resolve();
        case "get_prefs":
          return Promise.resolve({
            themePref: window.__MOCK_THEME__ || "light",
            onboarded: true,
            sampleText: "永远相信美好的事情即将发生",
            viewMode: "grid",
            sort: "name",
            soundPref: "off",
            transparencyPref: "full",
            uiScale: "auto",
          });
        case "get_settings":
          return Promise.resolve(settings);
        case "set_settings":
          settings = args.settings;
          return Promise.resolve();
        case "default_library_dir":
          return Promise.resolve("C:\\Users\\X\\AppData\\Local\\Microsoft\\Windows\\Fonts");
        case "scan_fonts":
          return Promise.resolve(fonts);
        case "get_tags":
          return Promise.resolve({
            Arial: ["无衬线", "网页"],
            Georgia: ["衬线"],
            Consolas: ["等宽", "代码"],
          });
        case "get_collections":
          return Promise.resolve({ "网页项目": ["Arial", "Verdana"] });
        case "get_favorites":
          return Promise.resolve(["Georgia"]);
        case "get_notes":
          return Promise.resolve({});
        case "get_ratings":
          return Promise.resolve({ Arial: 4, Georgia: 5 });
        case "get_aliases":
          return Promise.resolve({});
        case "list_trash":
          return Promise.resolve([]);
        case "font_usage":
          return Promise.resolve([]);
        case "audit_log":
        case "online_hosts":
          return Promise.resolve([]);
        case "adobe_available":
          return Promise.resolve(false);
        case "get_autostart":
          return Promise.resolve(false);
        case "set_ui_scale":
          return Promise.resolve(1);
        case "pinyin_keys":
          return Promise.resolve({});
        case "affinity_connection":
          return Promise.resolve({ reachable: false, version: null, docCount: 0, error: null });
        default:
          return Promise.resolve(null);
      }
    },
  };
})();
