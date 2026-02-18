import type { FC } from "hono/jsx";
import { LampCell } from "./LampCell";
import { LAMP_VALUES, getLampStyle } from "../lib/lamp";
import { formatTableKey } from "../lib/chart-table";
import type { TableEntry, TierGroup } from "../lib/chart-table";

interface TableViewProps {
  tableKey: string;
  tiers: TierGroup[];
  username: string;
}

export const TableView: FC<TableViewProps> = ({
  tableKey,
  tiers,
  username,
}) => {
  // Calculate statistics
  const allEntries = tiers.flatMap((t) => t.entries);
  const lampCounts = new Map<string, number>();
  for (const entry of allEntries) {
    lampCounts.set(entry.lamp, (lampCounts.get(entry.lamp) ?? 0) + 1);
  }

  return (
    <div>
      <h2 style="margin-bottom: 16px;">{formatTableKey(tableKey)}</h2>

      {/* Statistics bar */}
      <div class="card" id="stats-bar" style="margin-bottom: 24px;">
        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:8px;">
          {LAMP_VALUES.map((lamp) => {
            const style = getLampStyle(lamp);
            const count = lampCounts.get(lamp) ?? 0;
            return (
              <span
                data-lamp-stat={lamp}
                style={`font-size:0.85rem;padding:2px 8px;border-radius:3px;color:${style.color};background:${style.background.startsWith("linear") ? style.background : style.background}${style.border ? `;border:${style.border}` : ""}${count === 0 ? ";display:none" : ""}`}
              >
                {lamp}: <span class="lamp-count">{count}</span>
              </span>
            );
          })}
        </div>
        <div style="font-size:0.85rem;color:#666;">
          Total: <span data-total-count>{allEntries.length}</span>
        </div>
      </div>

      {/* Lamp filter */}
      <div style="margin-bottom: 16px; display: flex; gap: 16px; align-items: center;">
        <label style="display: flex; align-items: center; gap: 4px; cursor: pointer; font-size: 0.85rem; color: #ccc;">
          <input type="checkbox" id="filter-below-easy" />
          Below EASY
        </label>
        <label style="display: flex; align-items: center; gap: 4px; cursor: pointer; font-size: 0.85rem; color: #ccc;">
          <input type="checkbox" id="filter-below-hard" />
          Below HARD
        </label>
      </div>

      {/* Tier groups */}
      {tiers.map((tier) => (
        <div class="tier-group" style="margin-bottom: 20px;">
          <h3 style="font-size:0.85rem;color:#999;text-transform:uppercase;letter-spacing:0.05em;border-bottom:1px solid #2a2a2a;padding-bottom:4px;margin-bottom:8px;">
            {tier.tier}
          </h3>
          <div style="display:grid;grid-template-columns:repeat(auto-fill, minmax(200px, 1fr));gap:6px;">
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

      {/* Polling script */}
      <div
        id="table-data"
        data-username={username}
        data-lamp-styles={JSON.stringify(
          Object.fromEntries(
            LAMP_VALUES.map((l) => [l, getLampStyle(l)])
          )
        )}
        hidden
      />
      <script src="/table-filter.js"></script>
      <script src="/table-polling.js"></script>
    </div>
  );
};
