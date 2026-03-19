import * as fs from "node:fs/promises";
import * as path from "node:path";
import * as readline from "node:readline/promises";
import { fileURLToPath } from "node:url";
import { distance } from "fastest-levenshtein";

// --- Types ---

type Difficulty = "SPN" | "SPH" | "SPA" | "SPL";

type Lamp =
  | "NO PLAY"
  | "FAILED"
  | "ASSIST"
  | "EASY"
  | "CLEAR"
  | "HARD"
  | "EX HARD"
  | "FC";

interface TrackerEntry {
  songId: number;
  title: string;
  ratings: Record<Difficulty, number>;
  lamps: Record<Difficulty, Lamp>;
}

interface IidxApiEntry {
  title: string;
  tier: string;
  attributes: string[];
}

interface MappedEntry {
  songId: number;
  title: string;
  infinitasTitle: string;
  difficulty: Difficulty;
  tier: string;
  attributes: string[];
  sortOrder: number;
}

interface TitleMapping {
  "sp12-hard": MappedEntry[];
  "sp12-normal": MappedEntry[];
  "sp11-hard": MappedEntry[];
  "sp11-normal": MappedEntry[];
}

type EndpointKey = keyof TitleMapping;

// --- Constants ---

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const DEFAULT_TSV_PATH = path.resolve(__dirname, "../../.agent/tracker.tsv");
const OUTPUT_PATH = path.resolve(__dirname, "title-mapping.json");
const RESOLUTIONS_PATH = path.resolve(__dirname, "resolutions.json");

const ENDPOINTS: Record<EndpointKey, string> = {
  "sp11-normal": "https://dqn.github.io/iidxapi/sp11/normal.json",
  "sp11-hard": "https://dqn.github.io/iidxapi/sp11/hard.json",
  "sp12-normal": "https://dqn.github.io/iidxapi/sp12/normal.json",
  "sp12-hard": "https://dqn.github.io/iidxapi/sp12/hard.json",
};

const INFINITAS_MUSIC_URL = "https://dqn.github.io/iidxapi/infinitas/music.json";

const EXPECTED_RATING: Record<EndpointKey, number> = {
  "sp11-normal": 11,
  "sp11-hard": 11,
  "sp12-normal": 12,
  "sp12-hard": 12,
};

// TSV column indices (0-based)
const COL = {
  SONG_ID: 0,
  TITLE: 1,
  SPN_RATING: 18,
  SPN_LAMP: 19,
  SPH_RATING: 26,
  SPH_LAMP: 27,
  SPA_RATING: 34,
  SPA_LAMP: 35,
  SPL_RATING: 42,
  SPL_LAMP: 43,
} as const;

// Mojibake fixes: full title mapping (tracker title -> correct Unicode title)
const MOJIBAKE_FIXES: ReadonlyMap<string, string> = new Map([
  // Latin characters (accents, special characters)
  ["?bertreffen", "Übertreffen"],
  ["?THER", "ÆTHER"],
  ["?u Legends", "Ōu Legends"],
  ["?Viva!", "¡Viva!"],
  ["?影", "焱影"],
  ["ACT?", "ACTØ"],
  ["Amor De Ver?o", "Amor De Verão"],
  ["Dans la nuit de l'?ternit?", "Dans la nuit de l'éternité"],
  ["Geirsk?gul", "Geirskögul"],
  ["Ignis†Ir?", "Ignis†Iræ"],
  ["M?ch? M?nky", "Mächö Mönky"],
  ["P?rvat?", "Pārvatī"],
  ["POL?AMAИIA", "POLꓘAMAИIA"],
  ["Pr?ludium", "Präludium"],
  ["Raison d'?tre～交差する宿命～", "Raison d'être～交差する宿命～"],
  ["V?ID", "VØID"],
  ["旋律のドグマ～Mis?rables～", "旋律のドグマ～Misérables～"],
  ["u?n", "uən"],
  // Symbols (hearts)
  ["LOVE?SHINE", "LOVE♡SHINE"],
  ["Sweet Sweet?Magic", "Sweet Sweet♡Magic"],
  ["Raspberry?Heart(English version)", "Raspberry♡Heart(English version)"],
  ["Double??Loving Heart", "Double♡♡Loving Heart"],
  ["Love?km", "Love♥km"],
  ["超!!遠距離らぶ?メ～ル", "超!!遠距離らぶ♡メ～ル"],
  ["キャトられ?恋はモ～モク", "キャトられ♥恋はモ～モク"],
  ["表裏一体！？怪盗いいんちょの悩み?", "表裏一体！？怪盗いいんちょの悩み♥"],
  // Compound (multiple types of mojibake)
  ["?LOVE? シュガ→?", "♥LOVE² シュガ→♥"],
  ["ジオメトリック?ティーパーティー", "ジオメトリック∮ティーパーティー"],
]);

// Titles that legitimately contain '?'
const LEGITIMATE_QUESTION_MARKS = new Set([
  "Wanna Party?",
  "BLACK or WHITE?",
  "My Sweet Bird?",
  "がっつり陰キャ!? 怪盗いいんちょの億劫^^;",
]);

// Fullwidth to halfwidth character map
const FULLWIDTH_TO_HALFWIDTH: ReadonlyMap<string, string> = new Map([
  ["\uff5e", "~"], // ～ → ~
  ["\uff08", "("], // （ → (
  ["\uff09", ")"], // ） → )
  ["\uff01", "!"], // ！ → !
  ["\u3000", " "], // fullwidth space → halfwidth space
]);

// --- Normalization ---

function normalizeText(text: string): string {
  let result = "";
  for (const ch of text) {
    const mapped = FULLWIDTH_TO_HALFWIDTH.get(ch);
    if (mapped !== undefined) {
      result += mapped;
    } else {
      result += ch;
    }
  }
  return result.trim().toLowerCase();
}

// --- Suffix analysis ---

interface SuffixAnalysis {
  cleanTitle: string;
  difficulty: Difficulty;
}

function analyzeSuffix(title: string): SuffixAnalysis {
  if (title.endsWith("(L)")) {
    return {
      cleanTitle: title.slice(0, -3).trim(),
      difficulty: "SPL",
    };
  }
  if (title.endsWith("(H)")) {
    return {
      cleanTitle: title.slice(0, -3).trim(),
      difficulty: "SPH",
    };
  }
  return { cleanTitle: title, difficulty: "SPA" };
}

// --- Tracker loading (from TSV) ---

function parseLamp(value: string | undefined): Lamp {
  const trimmed = (value ?? "").trim();

  const validLamps: Lamp[] = [
    "NO PLAY",
    "FAILED",
    "ASSIST",
    "EASY",
    "CLEAR",
    "HARD",
    "EX HARD",
    "FC",
  ];
  if (validLamps.includes(trimmed as Lamp)) {
    return trimmed as Lamp;
  }
  return "NO PLAY";
}

async function loadTrackerFromTsv(
  tsvPath: string,
): Promise<Map<string, TrackerEntry>> {
  const content = await fs.readFile(tsvPath, "utf-8");
  const lines = content.split("\n");

  const entries = new Map<string, TrackerEntry>();
  let mojibakeFixed = 0;

  // Skip header line
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (line === undefined || line.trim() === "") {
      continue;
    }

    const cols = line.split("\t");
    let title = cols[COL.TITLE];
    if (title === undefined || title.trim() === "") {
      continue;
    }
    title = title.trim();

    const songId = Math.trunc(Number(cols[COL.SONG_ID] ?? "0"));
    if (!Number.isInteger(songId) || songId <= 0) {
      continue;
    }

    // Apply mojibake fix
    const fixed = MOJIBAKE_FIXES.get(title);
    if (fixed !== undefined) {
      console.log(`  Fixed: ${title} → ${fixed}`);
      title = fixed;
      mojibakeFixed++;
    }

    entries.set(title, {
      songId,
      title,
      ratings: {
        SPN: Math.trunc(Number(cols[COL.SPN_RATING] ?? "0")),
        SPH: Math.trunc(Number(cols[COL.SPH_RATING] ?? "0")),
        SPA: Math.trunc(Number(cols[COL.SPA_RATING] ?? "0")),
        SPL: Math.trunc(Number(cols[COL.SPL_RATING] ?? "0")),
      },
      lamps: {
        SPN: parseLamp(cols[COL.SPN_LAMP]),
        SPH: parseLamp(cols[COL.SPH_LAMP]),
        SPA: parseLamp(cols[COL.SPA_LAMP]),
        SPL: parseLamp(cols[COL.SPL_LAMP]),
      },
    });
  }

  console.log(`Mojibake fixed: ${mojibakeFixed}`);

  // Verify no remaining mojibake
  const remaining = [...entries.values()].filter(
    (e) =>
      e.title.includes("?") && !LEGITIMATE_QUESTION_MARKS.has(e.title),
  );
  if (remaining.length > 0) {
    console.warn(`WARNING: ${remaining.length} titles still contain '?':`);
    for (const e of remaining) {
      console.warn(`  - ${e.title}`);
    }
  }

  return entries;
}

// --- Matching ---

interface MatchResult {
  trackerTitle: string;
  songId: number;
  rating: number;
}

function findExactMatch(
  cleanTitle: string,
  difficulty: Difficulty,
  tracker: Map<string, TrackerEntry>,
): MatchResult | undefined {
  const entry = tracker.get(cleanTitle);
  if (entry === undefined) {
    return undefined;
  }
  return {
    trackerTitle: entry.title,
    songId: entry.songId,
    rating: entry.ratings[difficulty],
  };
}

function findNormalizedMatch(
  cleanTitle: string,
  difficulty: Difficulty,
  normalizedIndex: Map<string, TrackerEntry>,
): MatchResult | undefined {
  const normalizedKey = normalizeText(cleanTitle);
  const entry = normalizedIndex.get(normalizedKey);
  if (entry === undefined) {
    return undefined;
  }
  return {
    trackerTitle: entry.title,
    songId: entry.songId,
    rating: entry.ratings[difficulty],
  };
}

interface LevenshteinCandidate {
  songId: number;
  title: string;
  rating: number;
  distance: number;
}

function findLevenshteinCandidates(
  cleanTitle: string,
  difficulty: Difficulty,
  expectedRating: number,
  tracker: Map<string, TrackerEntry>,
  maxCandidates: number,
): LevenshteinCandidate[] {
  const normalizedInput = normalizeText(cleanTitle);
  const candidates: LevenshteinCandidate[] = [];

  for (const entry of tracker.values()) {
    // Only include candidates whose rating matches the expected value
    if (entry.ratings[difficulty] !== expectedRating) {
      continue;
    }
    const normalizedEntry = normalizeText(entry.title);
    const dist = distance(normalizedInput, normalizedEntry);
    candidates.push({
      songId: entry.songId,
      title: entry.title,
      rating: entry.ratings[difficulty],
      distance: dist,
    });
  }

  candidates.sort((a, b) => a.distance - b.distance);
  return candidates.slice(0, maxCandidates);
}

// --- Interactive resolution ---

async function resolveInteractively(
  apiTitle: string,
  difficulty: Difficulty,
  expectedRating: number,
  tracker: Map<string, TrackerEntry>,
  rl: readline.Interface,
): Promise<MatchResult | undefined> {
  const { cleanTitle } = analyzeSuffix(apiTitle);
  const candidates = findLevenshteinCandidates(
    cleanTitle,
    difficulty,
    expectedRating,
    tracker,
    10,
  );

  console.log(
    `\n\x1b[31m\u2717\x1b[0m No match: ${apiTitle} (${difficulty} \u2606${expectedRating})`,
  );
  console.log("  Candidates:");

  for (let i = 0; i < candidates.length; i++) {
    const c = candidates[i]!;
    console.log(
      `    ${i + 1}. ${c.title} (#${c.songId}, ${difficulty} Rating: ${c.rating}, dist: ${c.distance})`,
    );
  }
  console.log(`    ${candidates.length + 1}. Skip`);

  const answer = await rl.question("  Enter choice: ");
  const choice = Math.trunc(Number(answer));

  if (choice >= 1 && choice <= candidates.length) {
    const selected = candidates[choice - 1]!;
    return {
      trackerTitle: selected.title,
      songId: selected.songId,
      rating: selected.rating,
    };
  }

  return undefined;
}

// --- Persistent resolution cache ---

interface SavedResolution {
  trackerTitle: string;
  songId: number;
}

type ResolutionMap = Record<string, SavedResolution | null>;

async function loadResolutions(): Promise<ResolutionMap> {
  try {
    const content = await fs.readFile(RESOLUTIONS_PATH, "utf-8");
    return JSON.parse(content) as ResolutionMap;
  } catch {
    return {};
  }
}

async function saveResolutions(resolutions: ResolutionMap): Promise<void> {
  await fs.writeFile(RESOLUTIONS_PATH, JSON.stringify(resolutions, null, 2) + "\n");
}

// --- Fetch endpoints ---

async function fetchEndpoint(url: string): Promise<IidxApiEntry[]> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status}`);
  }
  return (await response.json()) as IidxApiEntry[];
}

// --- Main ---

export async function normalize(): Promise<void> {
  const tsvPath =
    process.argv[2] ?? process.env.INFST_TRACKER_TSV ?? DEFAULT_TSV_PATH;
  console.log(`Loading tracker TSV from ${tsvPath}...`);
  const tracker = await loadTrackerFromTsv(tsvPath);
  console.log(`Loaded ${tracker.size} songs`);

  // Fetch INFINITAS music list to filter out non-INFINITAS songs
  console.log("Fetching INFINITAS music list...");
  const infinitasMusic = await fetchEndpoint(INFINITAS_MUSIC_URL) as unknown as Array<{ title: string }>;
  const infinitasTitles = new Set<string>();
  const infinitasNormalized = new Set<string>();
  for (const m of infinitasMusic) {
    infinitasTitles.add(m.title);
    infinitasNormalized.add(normalizeText(m.title));
  }
  console.log(`INFINITAS has ${infinitasTitles.size} songs`);

  // Filter tracker to only INFINITAS songs
  let removedCount = 0;
  for (const [title, entry] of tracker) {
    if (!infinitasTitles.has(title) && !infinitasNormalized.has(normalizeText(title))) {
      tracker.delete(title);
      removedCount++;
    }
  }
  if (removedCount > 0) {
    console.log(`Filtered out ${removedCount} non-INFINITAS songs (${tracker.size} remaining)`);
  }

  // Build normalized index for fast lookup
  const normalizedIndex = new Map<string, TrackerEntry>();
  for (const entry of tracker.values()) {
    normalizedIndex.set(normalizeText(entry.title), entry);
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  const result: TitleMapping = {
    "sp12-hard": [],
    "sp12-normal": [],
    "sp11-hard": [],
    "sp11-normal": [],
  };

  // Phase 1: Fetch all endpoints and attempt automatic matching
  interface PendingEntry {
    key: EndpointKey;
    apiEntry: IidxApiEntry;
    apiIndex: number;
    cleanTitle: string;
    difficulty: Difficulty;
    expectedRating: number;
  }

  const pending: PendingEntry[] = [];

  // Counters for summary
  let autoMatched = 0;
  let notInInfinitas = 0;
  let chartNotAvailable = 0;
  let ratingMismatch = 0;

  for (const [key, url] of Object.entries(ENDPOINTS) as [
    EndpointKey,
    string,
  ][]) {
    const expectedRating = EXPECTED_RATING[key];
    console.log(`\n--- ${key} (fetching ${url}) ---`);
    const entries = await fetchEndpoint(url);
    console.log(`Fetched ${entries.length} entries`);

    for (let apiIndex = 0; apiIndex < entries.length; apiIndex++) {
      const apiEntry = entries[apiIndex]!;
      const { cleanTitle, difficulty } = analyzeSuffix(apiEntry.title);

      // 1. Try exact match
      let match = findExactMatch(cleanTitle, difficulty, tracker);

      // 2. Try normalized match
      if (match === undefined) {
        match = findNormalizedMatch(
          cleanTitle,
          difficulty,
          normalizedIndex,
        );
      }

      if (match !== undefined) {
        if (match.rating === expectedRating) {
          // Auto-match success
          console.log(
            `\x1b[32m\u2713\x1b[0m ${apiEntry.title} \u2192 ${match.trackerTitle} (${difficulty} \u2606${match.rating})`,
          );
          result[key].push({
            songId: match.songId,
            title: apiEntry.title,
            infinitasTitle: match.trackerTitle,
            difficulty,
            tier: apiEntry.tier,
            attributes: apiEntry.attributes,
            sortOrder: apiIndex,
          });
          autoMatched++;
        } else if (match.rating === 0) {
          // Chart not available in INFINITAS
          console.log(
            `\x1b[33m-\x1b[0m Chart not available: ${apiEntry.title} (${difficulty} not in tracker)`,
          );
          chartNotAvailable++;
        } else {
          // Rating mismatch
          console.log(
            `\x1b[33m!\x1b[0m Rating mismatch: ${apiEntry.title} -> ${match.trackerTitle} ` +
              `(expected ${difficulty} \u2606${expectedRating}, got \u2606${match.rating})`,
          );
          ratingMismatch++;
        }
      } else {
        // No match found — check if a close candidate exists with correct rating
        const candidates = findLevenshteinCandidates(
          cleanTitle,
          difficulty,
          expectedRating,
          tracker,
          1,
        );
        const bestCandidate = candidates[0];

        const maxLen = Math.max(
          normalizeText(cleanTitle).length,
          bestCandidate !== undefined
            ? normalizeText(bestCandidate.title).length
            : 0,
        );
        const ratio = maxLen > 0 && bestCandidate !== undefined
          ? bestCandidate.distance / maxLen
          : 1;

        if (bestCandidate !== undefined && ratio <= 0.30) {
          // Close match exists — needs interactive resolution
          pending.push({
            key,
            apiEntry,
            apiIndex,
            cleanTitle,
            difficulty,
            expectedRating,
          });
        } else {
          // Not in INFINITAS
          console.log(
            `\x1b[90m-\x1b[0m Not in INFINITAS: ${apiEntry.title}`,
          );
          notInInfinitas++;
        }
      }
    }
  }

  // Phase 2: Interactive resolution for unmatched entries
  if (pending.length === 0) {
    console.log("\nAll entries matched automatically!");
  } else {
    console.log(
      `\n\x1b[33m${pending.length} entries\x1b[0m need interactive resolution:`,
    );
    for (const p of pending) {
      console.log(`  - ${p.apiEntry.title} (${p.difficulty} \u2606${p.expectedRating}) [${p.key}]`);
    }
  }

  // Load persistent resolution cache from file
  const savedResolutions = await loadResolutions();
  // In-memory cache for this run (includes saved + new resolutions)
  const resolutionCache = new Map<string, MatchResult | undefined>();
  let savedHits = 0;

  // Pre-populate from saved resolutions
  for (const [key, value] of Object.entries(savedResolutions)) {
    if (value === null) {
      resolutionCache.set(key, undefined);
    } else {
      resolutionCache.set(key, {
        trackerTitle: value.trackerTitle,
        songId: value.songId,
        rating: 0, // not used after resolution
      });
    }
  }

  try {
    for (const p of pending) {
      const cacheKey = `${p.apiEntry.title}:${p.difficulty}`;
      let resolved: MatchResult | undefined;

      if (resolutionCache.has(cacheKey)) {
        resolved = resolutionCache.get(cacheKey);
        if (resolved !== undefined) {
          console.log(
            `\x1b[36m\u21bb\x1b[0m Cached: ${p.apiEntry.title} \u2192 ${resolved.trackerTitle} [${p.key}]`,
          );
        } else {
          console.log(`\x1b[33m-\x1b[0m Skipped (cached): ${p.apiEntry.title} [${p.key}]`);
        }
        savedHits++;
      } else {
        resolved = await resolveInteractively(
          p.apiEntry.title,
          p.difficulty,
          p.expectedRating,
          tracker,
          rl,
        );
        resolutionCache.set(cacheKey, resolved);

        // Save to persistent cache
        if (resolved !== undefined) {
          savedResolutions[cacheKey] = {
            trackerTitle: resolved.trackerTitle,
            songId: resolved.songId,
          };
          console.log(
            `\x1b[32m\u2713\x1b[0m Resolved: ${p.apiEntry.title} \u2192 ${resolved.trackerTitle}`,
          );
        } else {
          savedResolutions[cacheKey] = null;
          console.log(`\x1b[33m-\x1b[0m Skipped: ${p.apiEntry.title}`);
        }
      }

      if (resolved !== undefined) {
        result[p.key].push({
          songId: resolved.songId,
          title: p.apiEntry.title,
          infinitasTitle: resolved.trackerTitle,
          difficulty: p.difficulty,
          tier: p.apiEntry.tier,
          attributes: p.apiEntry.attributes,
          sortOrder: p.apiIndex,
        });
      }
    }
  } finally {
    rl.close();
  }

  // Save updated resolutions
  await saveResolutions(savedResolutions);
  if (savedHits > 0) {
    console.log(`Used ${savedHits} cached resolutions from ${RESOLUTIONS_PATH}`);
  }

  // Write output
  await fs.writeFile(OUTPUT_PATH, JSON.stringify(result, null, 2) + "\n");
  console.log(`\nWrote ${OUTPUT_PATH}`);

  // Summary
  console.log("\nSummary:");
  console.log(`  Auto-matched: ${autoMatched}`);
  console.log(`  Not in INFINITAS: ${notInInfinitas}`);
  console.log(`  Chart not available: ${chartNotAvailable}`);
  console.log(`  Rating mismatch: ${ratingMismatch}`);
  console.log(`  Need interactive resolution: ${pending.length}`);
  console.log("\nPer endpoint:");
  for (const [key, entries] of Object.entries(result) as [
    EndpointKey,
    MappedEntry[],
  ][]) {
    console.log(`  ${key}: ${entries.length} entries`);
  }
}

normalize();
