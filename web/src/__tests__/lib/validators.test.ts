import { describe, it, expect } from "vitest";

import { validateLoginInput, validateRegisterInput, validateLampInput } from "../../lib/validators";

describe("validateLoginInput", () => {
  it("rejects non-string inputs", () => {
    expect(validateLoginInput(undefined, "pass").valid).toBe(false);
    expect(validateLoginInput("a@b.c", undefined).valid).toBe(false);
  });

  it("rejects invalid email format", () => {
    const result = validateLoginInput("notanemail", "password");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Invalid email format");
  });

  it("rejects email without domain", () => {
    expect(validateLoginInput("user@", "password").valid).toBe(false);
  });

  it("accepts valid email and password", () => {
    expect(validateLoginInput("user@example.com", "password").valid).toBe(true);
  });
});

describe("validateRegisterInput", () => {
  it("rejects invalid email format", () => {
    const result = validateRegisterInput("bad", "password123", "user");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Invalid email format");
  });

  it("rejects short password", () => {
    const result = validateRegisterInput("a@b.com", "short", "user");
    expect(result.valid).toBe(false);
  });

  it("rejects invalid username", () => {
    const result = validateRegisterInput("a@b.com", "password123", "AB");
    expect(result.valid).toBe(false);
  });

  it("rejects reserved username", () => {
    const result = validateRegisterInput("a@b.com", "password123", "admin");
    expect(result.valid).toBe(false);
  });

  it("accepts valid input", () => {
    const result = validateRegisterInput("a@b.com", "password123", "validuser");
    expect(result.valid).toBe(true);
  });
});

describe("validateLampInput", () => {
  const valid = { songId: 1000, difficulty: "SPA", lamp: "HARD" };

  it("accepts valid input", () => {
    expect(validateLampInput(valid).valid).toBe(true);
  });

  it("accepts valid input with exScore and missCount", () => {
    expect(validateLampInput({ ...valid, exScore: 1500, missCount: 3 }).valid).toBe(true);
  });

  it("rejects non-integer songId", () => {
    expect(validateLampInput({ ...valid, songId: 1.5 }).valid).toBe(false);
  });

  it("rejects songId <= 0", () => {
    expect(validateLampInput({ ...valid, songId: 0 }).valid).toBe(false);
    expect(validateLampInput({ ...valid, songId: -1 }).valid).toBe(false);
  });

  it("rejects invalid difficulty", () => {
    expect(validateLampInput({ ...valid, difficulty: "INVALID" }).valid).toBe(false);
    expect(validateLampInput({ ...valid, difficulty: "spa" }).valid).toBe(false);
  });

  it("accepts all valid difficulties", () => {
    for (const d of ["SPB", "SPN", "SPH", "SPA", "SPL", "DPB", "DPN", "DPH", "DPA", "DPL"]) {
      expect(validateLampInput({ ...valid, difficulty: d }).valid).toBe(true);
    }
  });

  it("rejects invalid lamp", () => {
    expect(validateLampInput({ ...valid, lamp: "INVALID" }).valid).toBe(false);
  });

  it("rejects negative exScore", () => {
    const result = validateLampInput({ ...valid, exScore: -1 });
    expect(result.valid).toBe(false);
    expect(result.error).toContain("exScore");
  });

  it("rejects exScore over max", () => {
    const result = validateLampInput({ ...valid, exScore: 4001 });
    expect(result.valid).toBe(false);
  });

  it("rejects non-integer exScore", () => {
    expect(validateLampInput({ ...valid, exScore: 1.5 }).valid).toBe(false);
  });

  it("rejects string exScore", () => {
    expect(validateLampInput({ ...valid, exScore: "100" as unknown }).valid).toBe(false);
  });

  it("accepts exScore at boundaries", () => {
    expect(validateLampInput({ ...valid, exScore: 0 }).valid).toBe(true);
    expect(validateLampInput({ ...valid, exScore: 4000 }).valid).toBe(true);
  });

  it("rejects negative missCount", () => {
    const result = validateLampInput({ ...valid, missCount: -1 });
    expect(result.valid).toBe(false);
    expect(result.error).toContain("missCount");
  });

  it("rejects missCount over max", () => {
    expect(validateLampInput({ ...valid, missCount: 10001 }).valid).toBe(false);
  });

  it("rejects non-integer missCount", () => {
    expect(validateLampInput({ ...valid, missCount: 1.5 }).valid).toBe(false);
  });

  it("accepts missCount at boundaries", () => {
    expect(validateLampInput({ ...valid, missCount: 0 }).valid).toBe(true);
    expect(validateLampInput({ ...valid, missCount: 10000 }).valid).toBe(true);
  });
});
