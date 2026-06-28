// QMDC Mermaid bootstrap for MkDocs.
//
// Thin host adapter around the shared renderer in qmdc-mermaid-core.js (the
// single source of truth, also used by the VS Code preview). This file only
// does the MkDocs-specific wiring:
//
//   1. Self-host mermaid: load the vendored `js/mermaid.min.js` (a classic
//      script that sets the global `mermaid`). No third-party CDN — the site
//      works offline / behind a CSP, and there is no unpinned remote code
//      executed in every visitor's browser.
//   2. Theme: derive the mermaid theme from Material's color palette
//      (`<body data-md-color-scheme="slate">` → dark) and keep it in sync when
//      the user toggles the palette, re-rendering the diagrams.
//   3. Load the shared core, which reads the globals we set below.
//
// All the actual rendering + zoom/pan/toolbar logic lives in the core; do not
// duplicate it here.

(function () {
  "use strict";

  // Skip everything if the page has no diagrams.
  if (!document.querySelector(".mermaid")) return;

  var base = document.currentScript ? scriptDir(document.currentScript.src) : "";

  function scriptDir(src) {
    try {
      return src.slice(0, src.lastIndexOf("/") + 1);
    } catch (e) {
      return "";
    }
  }

  // Map Material's palette to a mermaid theme. 'slate' is Material's dark scheme.
  function themeForPalette() {
    var scheme =
      document.body && document.body.getAttribute("data-md-color-scheme");
    return scheme === "slate" ? "dark" : "default";
  }

  window.__qmdcMermaidTheme = themeForPalette();

  // Re-render on palette toggle. Material swaps data-md-color-scheme on <body>;
  // observe it and, when the derived theme actually changes, re-run the core's
  // renderer (passed to us as `run`) with the new theme.
  window.__qmdcMermaidOnReady = function (run) {
    if (typeof MutationObserver === "undefined" || !document.body) return;
    var current = window.__qmdcMermaidTheme;
    var mo = new MutationObserver(function () {
      var next = themeForPalette();
      if (next === current) return;
      current = next;
      window.__qmdcMermaidTheme = next;
      run();
    });
    mo.observe(document.body, {
      attributes: true,
      attributeFilter: ["data-md-color-scheme"],
    });
  };

  // Load a classic script and resolve when ready.
  function loadScript(src) {
    return new Promise(function (resolve, reject) {
      var s = document.createElement("script");
      s.src = src;
      s.onload = resolve;
      s.onerror = function () {
        reject(new Error("failed to load " + src));
      };
      document.head.appendChild(s);
    });
  }

  // Load vendored mermaid (sets global `mermaid`), then the shared core, which
  // auto-runs on load. If mermaid fails to load, the core is a no-op (it guards
  // on `typeof mermaid`), so diagrams just stay as plain text rather than break
  // the page.
  loadScript(base + "mermaid.min.js")
    .then(function () {
      return loadScript(base + "qmdc-mermaid-core.js");
    })
    .catch(function (err) {
      if (window.console) console.error("qmdc-mermaid bootstrap failed", err);
    });
})();
