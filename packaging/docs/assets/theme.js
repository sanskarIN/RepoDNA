// The theme chosen in the web version, light or dark, applied before the page is drawn. Without
// a choice, the page follows the system's theme.
(function () {
  try {
    var theme = window.localStorage.getItem("repodna-theme");
    if (theme === "light" || theme === "dark") {
      document.documentElement.dataset.theme = theme;
    }
  } catch (error) {
    // Storage can be off; the system's theme applies.
  }
})();
