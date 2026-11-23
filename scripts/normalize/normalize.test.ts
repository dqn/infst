import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import * as fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

interface Chart {
  songId: number;
  title: string;
  infinitasTitle: string;
  difficulty: string;
  tier: string;
  attributes: string[];
  sortOrder: number;
}

function trackerRow(
  songId: number,
  title: string,
  ratings: { normal?: number; hyper?: number; another?: number; leggendaria?: number },
): string {
  const columns = Array<string>(44).fill("");
  columns[0] = String(songId);
  columns[1] = title;
  columns[18] = String(ratings.normal ?? 0);
  columns[26] = String(ratings.hyper ?? 0);
  columns[34] = String(ratings.another ?? 0);
  columns[42] = String(ratings.leggendaria ?? 0);
  return columns.join("\t");
}

async function generateMapping(
  rows: string[],
  music: string[],
  tables: Record<string, Array<{ title: string; tier: string; attributes: string[] }>>,
): Promise<Record<string, Chart[]>> {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "infst-normalize-test-"));
  try {
    await fs.copyFile(path.join(scriptDir, "normalize.ts"), path.join(directory, "normalize.ts"));
    await fs.writeFile(path.join(directory, "package.json"), '{"type":"module"}\n');
    await fs.symlink(path.join(scriptDir, "node_modules"), path.join(directory, "node_modules"), "junction");
    await fs.writeFile(path.join(directory, "tracker.tsv"), ["header", ...rows].join("\n") + "\n");

    const responses = {
      "/iidxapi/infinitas/music.json": music.map((title) => ({ title })),
      ...Object.fromEntries(Object.entries(tables).map(([key, entries]) => [
        `/iidxapi/${key.replace("-", "/")}.json`, entries,
      ])),
    };
    await fs.writeFile(path.join(directory, "fetch-fixture.mjs"), `
      const responses = ${JSON.stringify(responses)};
      globalThis.fetch = async (url) => {
        const entries = responses[new URL(url).pathname];
        return entries === undefined
          ? new Response("Missing fixture", { status: 404 })
          : Response.json(entries);
      };
    `);

    const result = spawnSync(process.execPath, [
      "--import", "tsx", "--import", "./fetch-fixture.mjs", "normalize.ts", "tracker.tsv",
    ], { cwd: directory, encoding: "utf8", timeout: 30_000 });
    assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}\n${result.error ?? ""}`);
    return JSON.parse(await fs.readFile(path.join(directory, "title-mapping.json"), "utf8"));
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
}

test("includes INFINITAS-only charts as unranked without duplicating ranked charts", async () => {
  const result = await generateMapping([
    trackerRow(1001, "Ranked Song", { hyper: 11, another: 12 }),
    trackerRow(80044, "Cross Fire", { another: 12 }),
  ], ["Ranked Song", "Cross Fire"], {
    "sp11-normal": [{ title: "Ranked Song(H)", tier: "地力C", attributes: ["階段"] }],
    "sp11-hard": [{ title: "Ranked Song(H)", tier: "地力B", attributes: ["階段"] }],
    "sp12-normal": [{ title: "Ranked Song", tier: "地力A", attributes: ["皿"] }],
    "sp12-hard": [{ title: "Ranked Song", tier: "地力S", attributes: ["皿"] }],
  });

  assert.deepEqual(result["sp12-normal"], [
    { songId: 1001, title: "Ranked Song", infinitasTitle: "Ranked Song", difficulty: "SPA", tier: "地力A", attributes: ["皿"], sortOrder: 0 },
    { songId: 80044, title: "Cross Fire", infinitasTitle: "Cross Fire", difficulty: "SPA", tier: "未定", attributes: [], sortOrder: 1 },
  ]);
  assert.deepEqual(result["sp12-hard"], [
    { songId: 1001, title: "Ranked Song", infinitasTitle: "Ranked Song", difficulty: "SPA", tier: "地力S", attributes: ["皿"], sortOrder: 0 },
    { songId: 80044, title: "Cross Fire", infinitasTitle: "Cross Fire", difficulty: "SPA", tier: "未定", attributes: [], sortOrder: 1 },
  ]);
});

test("completes each table by chart difficulty while excluding unavailable and out-of-level charts", async () => {
  const result = await generateMapping([
    trackerRow(1001, "Mixed Charts", { hyper: 11, another: 12, leggendaria: 12 }),
    trackerRow(80001, "Rejection Girl", { another: 11 }),
    trackerRow(1002, "Arcade Only", { another: 12 }),
    trackerRow(1003, "Low Level", { another: 10 }),
    trackerRow(1004, "No Charts", {}),
  ], ["Mixed Charts", "Rejection Girl", "Low Level", "No Charts"], {
    "sp11-normal": [{ title: "Mixed Charts(H)", tier: "地力C", attributes: [] }],
    "sp11-hard": [],
    "sp12-normal": [{ title: "Mixed Charts(L)", tier: "地力S", attributes: [] }],
    "sp12-hard": [{ title: "Mixed Charts", tier: "地力A", attributes: [] }],
  });

  const charts = Object.fromEntries(Object.entries(result).map(([key, entries]) => [
    key, entries.map(({ title, infinitasTitle, difficulty, tier }) => ({ title, infinitasTitle, difficulty, tier })),
  ]));
  assert.deepEqual(charts, {
    "sp12-hard": [
      { title: "Mixed Charts", infinitasTitle: "Mixed Charts", difficulty: "SPA", tier: "地力A" },
      { title: "Mixed Charts(L)", infinitasTitle: "Mixed Charts", difficulty: "SPL", tier: "未定" },
    ],
    "sp12-normal": [
      { title: "Mixed Charts(L)", infinitasTitle: "Mixed Charts", difficulty: "SPL", tier: "地力S" },
      { title: "Mixed Charts", infinitasTitle: "Mixed Charts", difficulty: "SPA", tier: "未定" },
    ],
    "sp11-hard": [
      { title: "Mixed Charts(H)", infinitasTitle: "Mixed Charts", difficulty: "SPH", tier: "未定" },
      { title: "Rejection Girl", infinitasTitle: "Rejection Girl", difficulty: "SPA", tier: "未定" },
    ],
    "sp11-normal": [
      { title: "Mixed Charts(H)", infinitasTitle: "Mixed Charts", difficulty: "SPH", tier: "地力C" },
      { title: "Rejection Girl", infinitasTitle: "Rejection Girl", difficulty: "SPA", tier: "未定" },
    ],
  });
});
