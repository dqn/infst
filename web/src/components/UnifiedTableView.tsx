import type { FC } from "hono/jsx";
import { LampCell } from "./LampCell";
import { LAMP_VALUES, getLampStyle } from "../lib/lamp";
import { formatTableKey } from "../lib/chart-table";
import type { TierGroup } from "../lib/chart-table";

interface TableData {
  tableKey: string;
  tiers: TierGroup[];
}

interface UnifiedTableViewProps {
  tables: TableData[];
  username: string;
}

// Progress bar segments are ordered from highest lamp to lowest (left to right)
const PROGRESS_LAMP_ORDER = [...LAMP_VALUES].reverse();

export const UnifiedTableView: FC<UnifiedTableViewProps> = ({
  tables,
  username,
}) => {
  return (
    <div>
      {/* Summary */}
      <div id="unified-summary" class="card" style="margin-bottom:16px;">
        <h3 style="font-size:1rem;margin-bottom:12px;">Summary</h3>
        {(() => {
          // Group tables by level (e.g., sp11-normal + sp11-hard -> sp11)
          const levelGroups = new Map<string, typeof tables>();
          for (const table of tables) {
            const match = table.tableKey.match(/^(sp|dp)(\d+)-(normal|hard)$/);
            const levelKey = match ? `${match[1]}${match[2]}` : table.tableKey;
            const group = levelGroups.get(levelKey);
            if (group) {
              group.push(table);
            } else {
              levelGroups.set(levelKey, [table]);
            }
          }

          return Array.from(levelGroups.entries()).map(([levelKey, group]) => {
            // Deduplicate by songId:difficulty
            const seen = new Map<string, typeof group[0]["tiers"][0]["entries"][0]>();
            for (const t of group) {
              for (const tier of t.tiers) {
                for (const entry of tier.entries) {
                  const key = `${entry.songId}:${entry.difficulty}`;
                  if (!seen.has(key)) {
                    seen.set(key, entry);
                  }
                }
              }
            }
            const uniqueEntries = Array.from(seen.values());
            const total = uniqueEntries.length;
            const lampCounts = new Map<string, number>();
            for (const entry of uniqueEntries) {
              lampCounts.set(
                entry.lamp,
                (lampCounts.get(entry.lamp) ?? 0) + 1,
              );
            }

            const match = levelKey.match(/^(sp|dp)(\d+)$/);
            const label = match
              ? `${match[1].toUpperCase()}☆${match[2]}`
              : levelKey;

            return (
              <div class="summary-row" data-summary-level={levelKey}>
                <div class="summary-row-header">
                  <span class="table-name">{label}</span>
                  <span class="lamp-counts">
                    {LAMP_VALUES.filter(
                      (l) => (lampCounts.get(l) ?? 0) > 0,
                    ).map((lamp) => {
                      const style = getLampStyle(lamp);
                      return (
                        <span
                          data-summary-lamp={lamp}
                          style={`color:${style.background === "#333" ? "#888" : style.background}`}
                        >
                          {lamp}:{" "}
                          <span class="summary-lamp-count">
                            {lampCounts.get(lamp)}
                          </span>
                        </span>
                      );
                    })}
                  </span>
                </div>
                <div class="progress-bar-container">
                  {PROGRESS_LAMP_ORDER.filter(
                    (l) => (lampCounts.get(l) ?? 0) > 0,
                  ).map((lamp) => {
                    const count = lampCounts.get(lamp) ?? 0;
                    const pct = total > 0 ? (count / total) * 100 : 0;
                    const style = getLampStyle(lamp);
                    const bg = style.background.startsWith("linear")
                      ? style.background
                      : style.background;
                    return (
                      <div
                        class="progress-bar-segment"
                        data-lamp={lamp}
                        style={`width:${pct}%;background:${bg};`}
                        title={`${lamp}: ${count} (${pct.toFixed(1)}%)`}
                      />
                    );
                  })}
                </div>
              </div>
            );
          });
        })()}
      </div>

      {/* Table visibility toggles */}
      <div class="card" style="margin-bottom:16px;padding:12px 16px;">
        <div class="table-toggles">
          {tables.map((table) => (
            <label>
              <input
                type="checkbox"
                class="table-toggle"
                data-table-key={table.tableKey}
                checked
              />
              {formatTableKey(table.tableKey)}
            </label>
          ))}
        </div>
      </div>

      {/* Multi-column table view */}
      <div id="unified-tables" class="unified-columns">
        {tables.map((table) => {
          const allEntries = table.tiers.flatMap((t) => t.entries);
          const lampCounts = new Map<string, number>();
          for (const entry of allEntries) {
            lampCounts.set(
              entry.lamp,
              (lampCounts.get(entry.lamp) ?? 0) + 1,
            );
          }

          return (
            <div class="table-column" data-table-key={table.tableKey}>
              <div class="table-column-header">
                <h3>{formatTableKey(table.tableKey)}</h3>
                <div class="column-filters">
                  <label>
                    <input
                      type="checkbox"
                      class="filter-below-easy"
                      data-table-key={table.tableKey}
                    />
                    &lt;EASY
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      class="filter-below-hard"
                      data-table-key={table.tableKey}
                    />
                    &lt;HARD
                  </label>
                </div>
              </div>
              <div class="table-column-scroll">
                {/* Column stats */}
                <div class="column-stats" data-table-key={table.tableKey}>
                  {LAMP_VALUES.map((lamp) => {
                    const style = getLampStyle(lamp);
                    const count = lampCounts.get(lamp) ?? 0;
                    return (
                      <span
                        data-lamp-stat={lamp}
                        style={`padding:1px 6px;border-radius:3px;color:${style.color};background:${style.background}${style.border ? `;border:${style.border}` : ""}${count === 0 ? ";display:none" : ""}`}
                      >
                        {lamp}:{" "}
                        <span class="lamp-count">{count}</span>
                      </span>
                    );
                  })}
                  <span
                    style="color:#666;margin-left:4px;"
                    data-total-count
                  >
                    ({allEntries.length})
                  </span>
                </div>

                {/* Tier groups */}
                {table.tiers.map((tier) => (
                  <div class="tier-group">
                    <div class="tier-header">{tier.tier}</div>
                    <div class="tier-entries">
                      {tier.entries.map((entry) => (
                        <LampCell
                          songId={entry.songId}
                          title={entry.title}
                          difficulty={entry.difficulty}
                          lamp={entry.lamp}
                          attributes={entry.attributes}
                        />
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          );
        })}
      </div>

      {/* Data container for polling */}
      <div
        id="table-data"
        data-username={username}
        data-lamp-styles={JSON.stringify(
          Object.fromEntries(
            LAMP_VALUES.map((l) => [l, getLampStyle(l)]),
          ),
        )}
        hidden
      />
      <script src="/unified-table.js"></script>
      <script src="/table-polling.js"></script>
    </div>
  );
};
