// Session
export const SESSION_MAX_AGE_SECONDS = 604800; // 7 days

// Device code flow
export const DEVICE_CODE_EXPIRY_MS = 300_000; // 5 minutes
export const POLLING_INTERVAL_SECONDS = 5;

// Password constraints
export const PASSWORD_MIN_LENGTH = 8;
export const PASSWORD_MAX_LENGTH = 72;

// Email constraints
export const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

// Username constraints
export const USERNAME_MIN_LENGTH = 3;
export const USERNAME_MAX_LENGTH = 20;
export const USERNAME_PATTERN = /^[a-z0-9_-]{3,20}$/;

// Reserved usernames
export const RESERVED_USERNAMES = [
  "login",
  "register",
  "settings",
  "auth",
  "api",
  "admin",
  "guide",
  "icons",
];

// Rate limiting
export const RATE_LIMIT_LOGIN_MAX = 5;
export const RATE_LIMIT_LOGIN_WINDOW_SECONDS = 60;
export const RATE_LIMIT_REGISTER_MAX = 3;
export const RATE_LIMIT_REGISTER_WINDOW_SECONDS = 3600;
export const RATE_LIMIT_DEVICE_CODE_MAX = 10;
export const RATE_LIMIT_DEVICE_CODE_WINDOW_SECONDS = 60;
export const RATE_LIMIT_DEVICE_TOKEN_MAX = 60;
export const RATE_LIMIT_DEVICE_TOKEN_WINDOW_SECONDS = 300;

// API token expiry
export const API_TOKEN_EXPIRY_DAYS = 90;

// Valid difficulties
export const VALID_DIFFICULTIES = [
  "SPB", "SPN", "SPH", "SPA", "SPL",
  "DPB", "DPN", "DPH", "DPA", "DPL",
] as const;

// Score constraints
export const EX_SCORE_MAX = 4000;
export const MISS_COUNT_MAX = 10000;

// Bulk API
export const BULK_MAX_ENTRIES = 10_000;

// Cleanup
export const CLEANUP_PROBABILITY = 0.1; // 10% chance per poll
