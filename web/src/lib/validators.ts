import { isValidLamp } from "./lamp";
import {
  EMAIL_PATTERN,
  PASSWORD_MIN_LENGTH,
  PASSWORD_MAX_LENGTH,
  USERNAME_PATTERN,
  RESERVED_USERNAMES,
  VALID_DIFFICULTIES,
  EX_SCORE_MAX,
  MISS_COUNT_MAX,
} from "./constants";

interface ValidationResult {
  valid: boolean;
  error?: string;
}

export function validateLoginInput(email: unknown, password: unknown): ValidationResult {
  if (typeof email !== "string" || typeof password !== "string") {
    return { valid: false, error: "Email and password are required" };
  }
  if (!EMAIL_PATTERN.test(email)) {
    return { valid: false, error: "Invalid email format" };
  }
  return { valid: true };
}

export function validateRegisterInput(
  email: unknown,
  password: unknown,
  username: unknown,
): ValidationResult {
  if (
    typeof email !== "string" ||
    typeof password !== "string" ||
    typeof username !== "string"
  ) {
    return { valid: false, error: "All fields are required" };
  }

  if (!EMAIL_PATTERN.test(email)) {
    return { valid: false, error: "Invalid email format" };
  }

  if (password.length < PASSWORD_MIN_LENGTH || password.length > PASSWORD_MAX_LENGTH) {
    return {
      valid: false,
      error: `Password must be ${PASSWORD_MIN_LENGTH}-${PASSWORD_MAX_LENGTH} characters`,
    };
  }

  const trimmed = username.trim().toLowerCase();
  if (!USERNAME_PATTERN.test(trimmed)) {
    return {
      valid: false,
      error: "Username must be 3-20 characters (a-z, 0-9, -, _)",
    };
  }

  if (RESERVED_USERNAMES.includes(trimmed)) {
    return { valid: false, error: "This username is not available" };
  }

  return { valid: true };
}

interface LampInput {
  songId?: unknown;
  difficulty?: unknown;
  lamp?: unknown;
  exScore?: unknown;
  missCount?: unknown;
}

export function validateLampInput(entry: LampInput): ValidationResult {
  if (
    typeof entry.songId !== "number" ||
    !Number.isInteger(entry.songId) ||
    entry.songId <= 0 ||
    typeof entry.difficulty !== "string" ||
    typeof entry.lamp !== "string"
  ) {
    return {
      valid: false,
      error: "songId, difficulty, and lamp are required",
    };
  }

  if (!(VALID_DIFFICULTIES as readonly string[]).includes(entry.difficulty)) {
    return { valid: false, error: "Invalid difficulty value" };
  }

  if (!isValidLamp(entry.lamp)) {
    return { valid: false, error: "Invalid lamp value" };
  }

  if (entry.exScore !== undefined) {
    if (typeof entry.exScore !== "number" || !Number.isInteger(entry.exScore) || entry.exScore < 0 || entry.exScore > EX_SCORE_MAX) {
      return { valid: false, error: `exScore must be an integer between 0 and ${EX_SCORE_MAX}` };
    }
  }

  if (entry.missCount !== undefined) {
    if (typeof entry.missCount !== "number" || !Number.isInteger(entry.missCount) || entry.missCount < 0 || entry.missCount > MISS_COUNT_MAX) {
      return { valid: false, error: `missCount must be an integer between 0 and ${MISS_COUNT_MAX}` };
    }
  }

  return { valid: true };
}
