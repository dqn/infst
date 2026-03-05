(function () {
  var STORAGE_KEY = "infst-table-settings";
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
  // Reversed order for progress bars (highest first)
  var LAMP_NAMES = [
    "FC",
    "EX HARD",
    "HARD",
    "CLEAR",
    "EASY",
    "ASSIST",
    "FAILED",
    "NO PLAY",
  ];

  function loadSettings() {
    try {
      var raw = localStorage.getItem(STORAGE_KEY);
      return raw ? JSON.parse(raw) : {};
    } catch (e) {
      return {};
    }
  }

  function saveSettings(settings) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch (e) {
      // localStorage unavailable
    }
  }

  // --- Table visibility toggles ---
  function initToggles() {
    var settings = loadSettings();
    var hidden = settings.hiddenTables || [];

    document.querySelectorAll(".table-toggle").forEach(function (cb) {
      var key = cb.dataset.tableKey;
      if (hidden.indexOf(key) !== -1) {
        cb.checked = false;
        var col = document.querySelector(
          '.table-column[data-table-key="' + key + '"]',
        );
        if (col) col.classList.add("hidden-column");
      }

      cb.addEventListener("change", function () {
        var col = document.querySelector(
          '.table-column[data-table-key="' + key + '"]',
        );
        if (col) col.classList.toggle("hidden-column", !this.checked);

        var current = loadSettings();
        var hiddenSet = {};
        (current.hiddenTables || []).forEach(function (k) {
          hiddenSet[k] = true;
        });
        if (this.checked) {
          delete hiddenSet[key];
        } else {
          hiddenSet[key] = true;
        }
        current.hiddenTables = Object.keys(hiddenSet);
        saveSettings(current);
      });
    });
  }

  // --- Per-column lamp filters ---
  function applyFilter(tableKey) {
    var col = document.querySelector(
      '.table-column[data-table-key="' + tableKey + '"]',
    );
    if (!col) return;

    var easyCheckbox = col.querySelector(".filter-below-easy");
    var hardCheckbox = col.querySelector(".filter-below-hard");
    var belowEasy = easyCheckbox && easyCheckbox.checked;
    var belowHard = hardCheckbox && hardCheckbox.checked;

    var threshold = -1;
    if (belowEasy) {
      threshold = LAMP_ORDER.EASY;
    } else if (belowHard) {
      threshold = LAMP_ORDER.HARD;
    }

    var cells = col.querySelectorAll(".lamp-cell");
    var visibleCounts = {};
    var totalVisible = 0;

    cells.forEach(function (cell) {
      var lamp = cell.dataset.lamp || "NO PLAY";
      var order =
        LAMP_ORDER[lamp] !== undefined ? LAMP_ORDER[lamp] : -1;
      if (threshold > 0 && order >= threshold) {
        cell.style.display = "none";
      } else {
        cell.style.display = "";
        visibleCounts[lamp] = (visibleCounts[lamp] || 0) + 1;
        totalVisible++;
      }
    });

    // Update column stats
    col.querySelectorAll("[data-lamp-stat]").forEach(function (badge) {
      var lamp = badge.dataset.lampStat;
      var count = visibleCounts[lamp] || 0;
      var countEl = badge.querySelector(".lamp-count");
      if (countEl) countEl.textContent = count;
      badge.style.display = count > 0 ? "" : "none";
    });

    var totalEl = col.querySelector("[data-total-count]");
    if (totalEl) totalEl.textContent = "(" + totalVisible + ")";

    // Hide empty tier groups
    col.querySelectorAll(".tier-group").forEach(function (group) {
      var visible = group.querySelectorAll(
        '.lamp-cell:not([style*="display: none"])',
      );
      group.style.display = visible.length > 0 ? "" : "none";
    });
  }

  function initFilters() {
    var settings = loadSettings();
    var filters = settings.filters || {};

    document.querySelectorAll(".table-column").forEach(function (col) {
      var key = col.dataset.tableKey;
      var saved = filters[key] || {};

      var easyCheckbox = col.querySelector(".filter-below-easy");
      var hardCheckbox = col.querySelector(".filter-below-hard");

      if (easyCheckbox && saved.belowEasy) easyCheckbox.checked = true;
      if (hardCheckbox && saved.belowHard) hardCheckbox.checked = true;

      function onChange() {
        applyFilter(key);
        updateSummaryForLevel(tableKeyToLevelKey(key));
        var current = loadSettings();
        if (!current.filters) current.filters = {};
        current.filters[key] = {
          belowEasy: easyCheckbox ? easyCheckbox.checked : false,
          belowHard: hardCheckbox ? hardCheckbox.checked : false,
        };
        saveSettings(current);
      }

      if (easyCheckbox) easyCheckbox.addEventListener("change", onChange);
      if (hardCheckbox) hardCheckbox.addEventListener("change", onChange);

      // Apply initial filter if saved
      if (saved.belowEasy || saved.belowHard) {
        applyFilter(key);
      }
    });
  }

  // --- Summary updates ---
  function tableKeyToLevelKey(tableKey) {
    var match = tableKey.match(/^(sp|dp)(\d+)-(normal|hard)$/);
    return match ? match[1] + match[2] : tableKey;
  }

  function updateSummaryForLevel(levelKey) {
    var summaryRow = document.querySelector(
      '[data-summary-level="' + levelKey + '"]',
    );
    if (!summaryRow) return;

    // Aggregate lamp counts across all columns, deduplicating by songId:difficulty
    var seen = {};
    var counts = {};
    var total = 0;
    document.querySelectorAll(".table-column").forEach(function (col) {
      if (tableKeyToLevelKey(col.dataset.tableKey) !== levelKey) return;
      col.querySelectorAll(".lamp-cell").forEach(function (cell) {
        var key = cell.dataset.key;
        if (!key || seen[key]) return;
        seen[key] = true;
        var lamp = cell.dataset.lamp || "NO PLAY";
        counts[lamp] = (counts[lamp] || 0) + 1;
        total++;
      });
    });

    // Update lamp count text
    var countsContainer = summaryRow.querySelector(".lamp-counts");
    if (countsContainer) {
      countsContainer.querySelectorAll("[data-summary-lamp]").forEach(
        function (el) {
          var lamp = el.dataset.summaryLamp;
          var count = counts[lamp] || 0;
          var countEl = el.querySelector(".summary-lamp-count");
          if (countEl) countEl.textContent = count;
          el.style.display = count > 0 ? "" : "none";
        },
      );
    }

    // Update progress bar segments
    var bar = summaryRow.querySelector(".progress-bar-container");
    if (bar) {
      bar.querySelectorAll(".progress-bar-segment").forEach(function (seg) {
        var lamp = seg.dataset.lamp;
        var count = counts[lamp] || 0;
        var pct = total > 0 ? (count / total) * 100 : 0;
        seg.style.width = pct + "%";
        seg.title = lamp + ": " + count + " (" + pct.toFixed(1) + "%)";
        seg.style.display = count > 0 ? "" : "none";
      });
    }
  }

  function updateAllSummaries() {
    document.querySelectorAll("[data-summary-level]").forEach(function (row) {
      updateSummaryForLevel(row.dataset.summaryLevel);
    });
  }

  // --- Expose global for polling ---
  window.__tableFilterApply = function () {
    document.querySelectorAll(".table-column").forEach(function (col) {
      applyFilter(col.dataset.tableKey);
    });
    updateAllSummaries();
  };

  // --- Init ---
  initToggles();
  initFilters();
  // Apply initial summary (in case filters changed counts)
  updateAllSummaries();
})();
