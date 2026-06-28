// QMDC Extra JavaScript — scroll-spy, hint popovers, sidebar toggle, keyboard shortcuts
// Vanilla JS, no dependencies.

(function () {
  "use strict";

  // ─── Scroll-spy: highlight current TOC link based on scroll position ───

  function initScrollSpy() {
    var tocLinks = document.querySelectorAll(".qmdc-sidebar a[href^='#']");
    if (!tocLinks.length) return;

    var headings = [];
    tocLinks.forEach(function (link) {
      var id = link.getAttribute("href").slice(1);
      var el = document.getElementById(id);
      if (el) headings.push({ id: id, el: el, link: link });
    });

    if (!headings.length) return;

    // Use IntersectionObserver for efficient scroll tracking
    var activeLink = null;

    var observer = new IntersectionObserver(
      function (entries) {
        // Find the topmost visible heading
        var visible = [];
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            visible.push(entry);
          }
        });

        if (visible.length === 0) return;

        // Pick the heading closest to the top of the viewport
        var topEntry = visible.reduce(function (best, entry) {
          return entry.boundingClientRect.top < best.boundingClientRect.top
            ? entry
            : best;
        });

        var targetId = topEntry.target.id;
        var match = headings.find(function (h) {
          return h.id === targetId;
        });

        if (match && match.link !== activeLink) {
          if (activeLink) activeLink.classList.remove("active");
          match.link.classList.add("active");
          activeLink = match.link;
        }
      },
      {
        rootMargin: "0px 0px -60% 0px",
        threshold: 0,
      }
    );

    headings.forEach(function (h) {
      observer.observe(h.el);
    });

    // Fallback: also handle scroll for edge cases (top of page)
    var ticking = false;
    window.addEventListener("scroll", function () {
      if (ticking) return;
      ticking = true;
      requestAnimationFrame(function () {
        ticking = false;
        if (window.scrollY < 50 && headings.length > 0) {
          // At top of page, highlight first heading
          if (activeLink !== headings[0].link) {
            if (activeLink) activeLink.classList.remove("active");
            headings[0].link.classList.add("active");
            activeLink = headings[0].link;
          }
        }
      });
    });
  }

  // ─── Sidebar toggle: Material handles mobile nav natively ───
  // No custom toggle needed — Material's drawer handles the secondary sidebar.

  function initSidebarToggle() {
    // No-op: Material's built-in responsive behavior handles sidebar visibility.
  }

  // ─── Hint popover: delegated click handler (works with instant navigation) ───

  function initHintPopovers() {
    // Use event delegation on document — works even after Material replaces content
    document.addEventListener("click", function (e) {
      var btn = e.target.closest("[data-hint-toggle]");
      if (btn) {
        e.stopPropagation();
        var targetId = btn.getAttribute("data-hint-toggle");
        var popover = document.getElementById("hint-popover-" + targetId);
        if (!popover) return;

        var isActive = popover.classList.toggle("active");
        popover.hidden = !isActive;
        btn.setAttribute("aria-expanded", isActive ? "true" : "false");

        // Close other open popovers
        if (isActive) {
          document.querySelectorAll(".qmdc-hint-popover.active").forEach(function (other) {
            if (other !== popover) {
              other.classList.remove("active");
              other.hidden = true;
              var otherBtn = other.parentNode.querySelector("[data-hint-toggle]");
              if (otherBtn) otherBtn.setAttribute("aria-expanded", "false");
            }
          });
        }
        return;
      }

      // Click outside any popover — close all
      if (!e.target.closest(".qmdc-hint-popover")) {
        document.querySelectorAll(".qmdc-hint-popover.active").forEach(function (popover) {
          popover.classList.remove("active");
          popover.hidden = true;
          var b = popover.parentNode.querySelector("[data-hint-toggle]");
          if (b) b.setAttribute("aria-expanded", "false");
        });
      }
    });
  }

  // ─── Header title → home (Material only links the logo icon, not the title) ───

  function initHeaderTitleLink() {
    var title = document.querySelector(".md-header__title");
    var logo = document.querySelector(
      ".md-header a.md-logo, .md-header [data-md-component='logo']"
    );
    if (!title || !logo) return;
    var href = logo.getAttribute("href");
    if (!href) return;
    title.style.cursor = "pointer";
    title.addEventListener("click", function (e) {
      // Leave real links/buttons inside the title (if any) alone.
      if (e.target.closest("a, button")) return;
      window.location.href = href;
    });
  }

  // ─── `/` keyboard shortcut: focus Material's native search input ───

  function initSearchShortcut() {
    document.addEventListener("keydown", function (e) {
      if (e.key !== "/") return;

      // Don't trigger when user is typing in a text field
      var tag = document.activeElement.tagName;
      if (
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        tag === "SELECT" ||
        document.activeElement.isContentEditable
      ) {
        return;
      }

      e.preventDefault();

      // Focus Material's native search input
      var searchInput = document.querySelector(".md-search__input");
      if (searchInput) {
        searchInput.focus();
      }
    });
  }

  // ─── Initialize all features on DOM ready ───

  function init() {
    initScrollSpy();
    initSidebarToggle();
    initHintPopovers();
    initHeaderTitleLink();
    initSearchShortcut();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
