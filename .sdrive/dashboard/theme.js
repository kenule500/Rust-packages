/* Set the theme before first paint, so the page never flashes the wrong one.
   Order of precedence: saved choice -> dark. Dark is the default; the toggle
   writes an explicit light/dark choice to localStorage. Kept in its own file so
   the dashboard can ship a strict CSP. */
(function () {
  var theme = "dark";

  try {
    var saved = window.localStorage.getItem("sdrive-theme");

    if (saved === "light" || saved === "dark") {
      theme = saved;
    }
  } catch (error) {
    /* localStorage can be blocked; dark is the default. */
  }

  document.documentElement.setAttribute("data-theme", theme);
})();
