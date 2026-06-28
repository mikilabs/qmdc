// QMDC Mermaid core — natural-size rendering + zoom/pan viewport.
//
// ┌─────────────────────────────────────────────────────────────────────────┐
// │ SINGLE SOURCE OF TRUTH. Used by BOTH renderers:                           │
// │   • MkDocs SSG  — shipped verbatim as a site asset (qmdc-mkdocs).          │
// │   • VS Code preview — inlined into the webview HTML at runtime             │
// │                       (qmdc-vscode/src/preview-renderer.ts reads this file).│
// │ Do NOT fork this logic. The only host-specific bits are injected via       │
// │ globals set BEFORE this script runs (see "Host contract" below).           │
// └─────────────────────────────────────────────────────────────────────────┘
//
// Host contract (globals, all optional):
//   • window.mermaid              — the mermaid library (already loaded). If
//                                   absent, this script is a no-op.
//   • window.__qmdcMermaidTheme    — mermaid theme name ('dark' | 'default' | …).
//                                   Defaults to 'default'. VS Code sets 'dark';
//                                   MkDocs derives it from the Material palette.
//   • window.__qmdcMermaidOnReady  — optional callback(run) invoked once after the
//                                   first render, so the host can re-render later
//                                   (e.g. MkDocs palette toggle). `run` re-runs
//                                   mermaid + re-enhances.
//
// Responsibilities:
//   1. Initialize mermaid with `useMaxWidth: false` for every diagram type (and
//      `wrap: true` for sequence diagrams), so SVGs render at their *natural* size
//      and long labels wrap, instead of a wide diagram being squished — unreadable,
//      with no way to zoom. securityLevel is pinned to 'strict' (mermaid's default)
//      so author diagram source cannot execute script.
//   2. After rendering, wrap each diagram in a viewport that fits the diagram to
//      the column width and scrolls *horizontally* when zoomed wider. Height
//      follows the SVG, so the diagram is never clipped vertically — the page just
//      scrolls. A toolbar offers zoom in/out, fit-to-width and actual size;
//      Ctrl/Cmd+wheel zooms toward the cursor; drag and arrow keys pan.

(function () {
  "use strict";

  if (typeof mermaid === "undefined") return;

  function clamp(v, lo, hi) {
    return Math.max(lo, Math.min(hi, v));
  }

  function enhance(container) {
    if (container.dataset.zoomReady) return;
    var svg = container.querySelector("svg");
    if (!svg) return;
    // Only enhance a normally-rendered diagram, where the SVG is a direct child
    // of the .mermaid container. Mermaid's *error* output nests the SVG
    // differently; skip those so a broken diagram is left as-is.
    if (svg.parentNode !== container) return;
    container.dataset.zoomReady = "1";

    // Natural (intrinsic) diagram size from the viewBox, falling back to bbox.
    var vb = svg.viewBox && svg.viewBox.baseVal;
    var rectS = svg.getBoundingClientRect();
    var natW = (vb && vb.width) || rectS.width || 100;
    var natH = (vb && vb.height) || rectS.height || 100;

    container.style.position = "relative";

    // Scroll wrapper — only scrolls horizontally; height follows the scaled SVG.
    var wrap = document.createElement("div");
    wrap.className = "mermaid-viewport";
    container.insertBefore(wrap, svg);
    wrap.appendChild(svg);

    svg.style.maxWidth = "none";
    svg.style.display = "block";

    var scale = 1;
    var label = null;

    function availWidth() {
      return wrap.clientWidth || container.clientWidth || natW;
    }

    function apply() {
      svg.style.width = natW * scale + "px";
      svg.style.height = natH * scale + "px";
      if (label) label.textContent = Math.round(scale * 100) + "%";
    }

    // Default + "fit to width": scale so the diagram fills the column width
    // without upscaling. Full height stays visible; the page scrolls if tall.
    function fit() {
      scale = Math.min(availWidth() / natW, 1);
      apply();
    }

    function actualSize() {
      scale = 1;
      apply();
    }

    // Zoom keeping the point under the cursor fixed horizontally.
    function zoomAt(clientX, factor) {
      var prev = scale;
      var ns = clamp(scale * factor, 0.1, 8);
      if (ns === prev) return;
      var r = wrap.getBoundingClientRect();
      var cursorX = clientX - r.left;
      var contentX = (wrap.scrollLeft + cursorX) / prev;
      scale = ns;
      apply();
      wrap.scrollLeft = contentX * ns - cursorX;
    }

    // Toolbar
    var bar = document.createElement("div");
    bar.className = "mermaid-toolbar";
    function mkBtn(text, title) {
      var b = document.createElement("button");
      b.className = "mermaid-btn";
      b.type = "button";
      b.textContent = text;
      b.title = title;
      // title is only a tooltip; set an explicit accessible name too (the
      // glyphs, esp. U+2922, announce poorly to screen readers).
      b.setAttribute("aria-label", title);
      bar.appendChild(b);
      return b;
    }
    var bOut = mkBtn("\u2212", "Zoom out (Ctrl/Cmd + scroll)");
    label = document.createElement("span");
    label.className = "mermaid-zoom-label";
    bar.appendChild(label);
    var bIn = mkBtn("+", "Zoom in (Ctrl/Cmd + scroll)");
    var bFit = mkBtn("\u2922", "Fit to width");
    var bOne = mkBtn("1:1", "Actual size");
    container.appendChild(bar);

    // Keep the grab cursor / pannable affordance only while the diagram
    // actually overflows the viewport horizontally.
    function updateOverflowState() {
      var overflows = wrap.scrollWidth > wrap.clientWidth + 1;
      wrap.classList.toggle("mermaid-pannable", overflows);
    }

    // "fitted" tracks whether we should re-fit on container resize.
    // Zooming/1:1 opt out; Fit opts back in.
    var fitted = true;

    function centerZoom(factor) {
      var r = wrap.getBoundingClientRect();
      zoomAt(r.left + r.width / 2, factor);
    }
    bIn.addEventListener("click", function (e) {
      e.stopPropagation();
      fitted = false;
      centerZoom(1.25);
      updateOverflowState();
    });
    bOut.addEventListener("click", function (e) {
      e.stopPropagation();
      fitted = false;
      centerZoom(0.8);
      updateOverflowState();
    });
    bFit.addEventListener("click", function (e) {
      e.stopPropagation();
      fitted = true;
      fit();
      updateOverflowState();
    });
    bOne.addEventListener("click", function (e) {
      e.stopPropagation();
      fitted = false;
      actualSize();
      updateOverflowState();
    });

    // Wheel zoom toward cursor — only with Ctrl/Cmd held, so a plain wheel
    // still scrolls the page normally.
    wrap.addEventListener(
      "wheel",
      function (e) {
        if (!e.ctrlKey && !e.metaKey) return;
        e.preventDefault();
        fitted = false;
        zoomAt(e.clientX, e.deltaY < 0 ? 1.1 : 0.9);
        updateOverflowState();
      },
      { passive: false }
    );

    // Keyboard: focusable so horizontal pan is reachable without a pointer.
    // Arrow keys scroll, +/- zoom, 0 fits.
    wrap.tabIndex = 0;
    wrap.addEventListener("keydown", function (e) {
      var step = 60;
      if (e.key === "ArrowRight") {
        wrap.scrollLeft += step;
        e.preventDefault();
      } else if (e.key === "ArrowLeft") {
        wrap.scrollLeft -= step;
        e.preventDefault();
      } else if (e.key === "+" || e.key === "=") {
        fitted = false;
        centerZoom(1.25);
        updateOverflowState();
        e.preventDefault();
      } else if (e.key === "-") {
        fitted = false;
        centerZoom(0.8);
        updateOverflowState();
        e.preventDefault();
      } else if (e.key === "0") {
        fitted = true;
        fit();
        updateOverflowState();
        e.preventDefault();
      }
    });

    // Drag to pan horizontally (only meaningful once zoomed wider than column).
    var dragging = false,
      startX = 0,
      startScroll = 0;
    wrap.addEventListener("pointerdown", function (e) {
      if (e.target.closest(".mermaid-toolbar")) return;
      dragging = true;
      startX = e.clientX;
      startScroll = wrap.scrollLeft;
      wrap.classList.add("grabbing");
      try {
        wrap.setPointerCapture(e.pointerId);
      } catch (err) {
        /* ignore */
      }
    });
    wrap.addEventListener("pointermove", function (e) {
      if (!dragging) return;
      wrap.scrollLeft = startScroll - (e.clientX - startX);
    });
    function endDrag() {
      dragging = false;
      wrap.classList.remove("grabbing");
    }
    wrap.addEventListener("pointerup", endDrag);
    wrap.addEventListener("pointercancel", endDrag);
    wrap.addEventListener("pointerleave", endDrag);

    // Re-fit on container resize (e.g. window/panel resize) while at fit scale.
    if (typeof ResizeObserver !== "undefined") {
      var ro = new ResizeObserver(function () {
        if (fitted) fit();
        updateOverflowState();
      });
      ro.observe(container);
    }

    fit();
    updateOverflowState();
  }

  function enhanceAll() {
    document.querySelectorAll(".mermaid").forEach(function (el) {
      try {
        enhance(el);
      } catch (e) {
        if (window.console) console.error("mermaid enhance failed", e);
      }
    });
  }

  // Stash each diagram's original source once, and reset any already-rendered
  // diagram back to that source. Makes run() safely repeatable — needed for the
  // MkDocs palette toggle (re-theme) and SPA-style re-navigation.
  function resetDiagrams() {
    document.querySelectorAll(".mermaid").forEach(function (el) {
      if (el.dataset.qmdcSrc == null) {
        el.dataset.qmdcSrc = (el.textContent || "").trim();
        return;
      }
      // Already rendered before: restore source. Setting textContent wipes the
      // generated SVG, the .mermaid-viewport wrapper and the .mermaid-toolbar
      // (all children), so the next mermaid.run starts from a clean slate.
      el.textContent = el.dataset.qmdcSrc;
      delete el.dataset.processed;
      delete el.dataset.zoomReady;
    });
  }

  function run() {
    var NOMAX = { useMaxWidth: false };
    mermaid.initialize({
      startOnLoad: false,
      // securityLevel 'strict' (mermaid's default, set explicitly): labels are
      // HTML-encoded and click/script directives disabled, so author diagram
      // source cannot execute script. <br/> labels still render (mermaid uses
      // foreignObject regardless).
      securityLevel: "strict",
      theme: window.__qmdcMermaidTheme || "default",
      // wrap: true makes sequence diagrams break long message/note text onto
      // multiple lines instead of stretching the diagram. Short labels are
      // unaffected; scoped to the sequence namespace so flowcharts etc. keep
      // their own (often intentional br-based) layout.
      sequence: { useMaxWidth: false, wrap: true },
      flowchart: NOMAX,
      gantt: NOMAX,
      er: NOMAX,
      journey: NOMAX,
      class: NOMAX,
      state: NOMAX,
      pie: NOMAX,
    });

    resetDiagrams();

    // suppressErrors keeps one malformed diagram from rejecting the whole batch.
    // enhance() skips any .mermaid that has no <svg>, so failed diagrams are
    // simply left as-is.
    try {
      var p = mermaid.run({ querySelector: ".mermaid", suppressErrors: true });
      if (p && typeof p.finally === "function") {
        p.finally(enhanceAll);
      } else if (p && typeof p.then === "function") {
        p.then(enhanceAll, enhanceAll);
      } else {
        enhanceAll();
      }
    } catch (err) {
      if (window.console) console.error("mermaid render failed", err);
      enhanceAll();
    }
  }

  function start() {
    run();
    // Let the host re-render later (e.g. MkDocs palette toggle). The host is
    // responsible for clearing already-rendered diagrams before calling run().
    if (typeof window.__qmdcMermaidOnReady === "function") {
      try {
        window.__qmdcMermaidOnReady(run);
      } catch (e) {
        if (window.console) console.error("mermaid onReady failed", e);
      }
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
