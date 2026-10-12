// The documentation site: the menu on narrow screens, the theme button, copy buttons on code,
// and search over every page.
(function () {
  "use strict";

  var html = document.documentElement;
  var root = html.dataset.root || "./";

  // The menu, on screens too narrow for the sidebar.
  var menu = document.querySelector(".menu");
  var sidebar = document.getElementById("sidebar");
  function setMenu(open) {
    document.body.classList.toggle("nav-open", open);
    menu.setAttribute("aria-expanded", String(open));
  }
  if (menu && sidebar) {
    menu.addEventListener("click", function () {
      setMenu(!document.body.classList.contains("nav-open"));
    });
    sidebar.addEventListener("click", function (event) {
      if (event.target.closest("a")) setMenu(false);
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && document.body.classList.contains("nav-open")) {
        setMenu(false);
        menu.focus();
      }
    });
    var current = sidebar.querySelector('[aria-current="page"]');
    if (current) current.scrollIntoView({ block: "nearest" });
  }

  // The theme: the system's, light, or dark. The web version remembers the same choice.
  var KEY = "repodna-theme";
  var NAMES = { system: "System", light: "Light", dark: "Dark" };
  var themeButton = document.querySelector(".theme");
  function storedTheme() {
    try {
      var value = window.localStorage.getItem(KEY);
      return value === "light" || value === "dark" ? value : "system";
    } catch (error) {
      return "system";
    }
  }
  function showTheme(theme) {
    if (theme === "system") {
      delete html.dataset.theme;
    } else {
      html.dataset.theme = theme;
    }
    if (themeButton) {
      themeButton.textContent = NAMES[theme];
      themeButton.setAttribute("aria-label", "Theme: " + NAMES[theme].toLowerCase());
    }
  }
  if (themeButton) {
    showTheme(storedTheme());
    themeButton.addEventListener("click", function () {
      var next = { system: "light", light: "dark", dark: "system" }[storedTheme()];
      try {
        if (next === "system") window.localStorage.removeItem(KEY);
        else window.localStorage.setItem(KEY, next);
      } catch (error) {
        // Without storage the choice lasts until the page changes.
      }
      showTheme(next);
    });
  }

  // A copy button on every code block.
  document.querySelectorAll(".code").forEach(function (block) {
    var code = block.querySelector("code");
    if (!code || !navigator.clipboard) return;
    var button = document.createElement("button");
    button.type = "button";
    button.className = "copy";
    button.textContent = "Copy";
    button.addEventListener("click", function () {
      navigator.clipboard.writeText(code.textContent).then(
        function () {
          button.textContent = "Copied";
          setTimeout(function () {
            button.textContent = "Copy";
          }, 1600);
        },
        function () {
          button.textContent = "Select and copy";
        },
      );
    });
    block.appendChild(button);
  });

  // Search: every page, its sections, and their first words.
  var input = document.getElementById("search");
  var results = document.getElementById("results");
  if (!input || !results) return;
  var index = null;
  var loading = null;
  function load() {
    loading =
      loading ||
      fetch(root + "search-index.json")
        .then(function (response) {
          return response.json();
        })
        .then(function (data) {
          index = data;
        })
        .catch(function () {
          index = [];
        });
    return loading;
  }
  function score(text, terms, weight) {
    var lower = text.toLowerCase();
    var total = 0;
    for (var i = 0; i < terms.length; i += 1) {
      if (lower.indexOf(terms[i]) !== -1) total += weight;
    }
    return total;
  }
  // The words around the first term the text contains, beginning and ending on whole words.
  function snippet(text, terms) {
    var lower = text.toLowerCase();
    var at = -1;
    for (var i = 0; i < terms.length && at === -1; i += 1) at = lower.indexOf(terms[i]);
    var start = at === -1 ? 0 : Math.max(0, at - 50);
    var end = Math.min(text.length, at === -1 ? 140 : at + 110);
    if (start > 0) {
      var space = text.indexOf(" ", start);
      if (space !== -1 && space < at) start = space + 1;
    }
    if (end < text.length) {
      var last = text.lastIndexOf(" ", end);
      if (last > Math.max(start, at)) end = last;
    }
    return (start > 0 ? "…" : "") + text.slice(start, end) + (end < text.length ? "…" : "");
  }
  function search(query) {
    var terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0 || !index) return [];
    var found = [];
    index.forEach(function (page) {
      var all = page.s
        .map(function (section) {
          return section.t + " " + section.b;
        })
        .join(" ")
        .toLowerCase();
      var everyTerm = terms.every(function (term) {
        return all.indexOf(term) !== -1;
      });
      if (!everyTerm) return;
      page.s.forEach(function (section) {
        var top = section.i === "";
        var sectionScore =
          score(top ? page.t : section.t, terms, top ? 10 : 6) + score(section.b, terms, 1);
        if (sectionScore > 0) {
          found.push({
            score: sectionScore,
            title: top ? page.t : page.t + " › " + section.t,
            url: page.u + (top ? "" : "#" + section.i),
            text: snippet(section.b || page.d, terms),
          });
        }
      });
    });
    found.sort(function (a, b) {
      return b.score - a.score;
    });
    return found.slice(0, 10);
  }
  function show(list, query) {
    results.textContent = "";
    if (!query) {
      results.hidden = true;
      input.setAttribute("aria-expanded", "false");
      return;
    }
    if (list.length === 0) {
      var none = document.createElement("li");
      none.className = "none";
      none.textContent = "Nothing found for “" + query + "”.";
      results.appendChild(none);
    }
    list.forEach(function (result) {
      var item = document.createElement("li");
      var link = document.createElement("a");
      link.href = root + result.url;
      var title = document.createElement("strong");
      title.textContent = result.title;
      link.appendChild(title);
      if (result.text) {
        var text = document.createElement("span");
        text.textContent = result.text;
        link.appendChild(text);
      }
      item.appendChild(link);
      results.appendChild(item);
    });
    results.hidden = false;
    input.setAttribute("aria-expanded", "true");
  }
  function update() {
    var query = input.value.trim();
    load().then(function () {
      if (input.value.trim() === query) show(search(query), query);
    });
  }
  input.addEventListener("focus", load);
  input.addEventListener("input", update);
  input.addEventListener("keydown", function (event) {
    var links = results.querySelectorAll("a");
    if (event.key === "ArrowDown" && links.length > 0) {
      event.preventDefault();
      links[0].focus();
    } else if (event.key === "Enter" && links.length > 0) {
      event.preventDefault();
      links[0].click();
    } else if (event.key === "Escape") {
      input.value = "";
      show([], "");
    }
  });
  results.addEventListener("keydown", function (event) {
    var links = Array.prototype.slice.call(results.querySelectorAll("a"));
    var position = links.indexOf(document.activeElement);
    if (event.key === "ArrowDown" && position < links.length - 1) {
      event.preventDefault();
      links[position + 1].focus();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      if (position <= 0) input.focus();
      else links[position - 1].focus();
    } else if (event.key === "Escape") {
      input.focus();
      show([], "");
    }
  });
  document.addEventListener("click", function (event) {
    if (!event.target.closest(".search")) {
      results.hidden = true;
      input.setAttribute("aria-expanded", "false");
    }
  });
  document.addEventListener("keydown", function (event) {
    var typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement.tagName);
    if (event.key === "/" && !typing) {
      event.preventDefault();
      input.focus();
    }
  });
})();
