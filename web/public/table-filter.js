(function () {
  var LAMP_ORDER = {
    "NO PLAY": 0,
    FAILED: 1,
    ASSIST: 2,
    EASY: 3,
    CLEAR: 4,
    HARD: 5,
    "EX HARD": 6,
    FC: 7,
  };

  var filterEasy = document.getElementById("filter-below-easy");
  var filterHard = document.getElementById("filter-below-hard");
  if (!filterEasy || !filterHard) return;

  function apply() {
    var belowEasy = filterEasy.checked;
    var belowHard = filterHard.checked;

    // Determine the threshold: below EASY is stricter than below HARD
    var threshold = -1;
    if (belowEasy) {
      threshold = LAMP_ORDER.EASY; // show only order < 3
    } else if (belowHard) {
      threshold = LAMP_ORDER.HARD; // show only order < 5
    }

    var cells = document.querySelectorAll(".lamp-cell");
    // Track visible counts per lamp
    var visibleCounts = {};
    var totalVisible = 0;

    cells.forEach(function (cell) {
      var lamp = cell.dataset.lamp || "NO PLAY";
      var order = LAMP_ORDER[lamp] !== undefined ? LAMP_ORDER[lamp] : -1;

      if (threshold > 0 && order >= threshold) {
        cell.style.display = "none";
      } else {
        cell.style.display = "";
        visibleCounts[lamp] = (visibleCounts[lamp] || 0) + 1;
        totalVisible++;
      }
    });

    // Update statistics bar
    var statBadges = document.querySelectorAll("[data-lamp-stat]");
    statBadges.forEach(function (badge) {
      var lamp = badge.dataset.lampStat;
      var count = visibleCounts[lamp] || 0;
      var countEl = badge.querySelector(".lamp-count");
      if (countEl) countEl.textContent = count;
      badge.style.display = count > 0 ? "" : "none";
    });

    var totalEl = document.querySelector("[data-total-count]");
    if (totalEl) totalEl.textContent = totalVisible;

    // Hide tier groups with no visible cells
    var groups = document.querySelectorAll(".tier-group");
    groups.forEach(function (group) {
      var visibleCells = group.querySelectorAll(
        '.lamp-cell:not([style*="display: none"])'
      );
      group.style.display = visibleCells.length > 0 ? "" : "none";
    });
  }

  filterEasy.addEventListener("change", apply);
  filterHard.addEventListener("change", apply);

  // Expose globally for polling integration
  window.__tableFilterApply = apply;
})();
