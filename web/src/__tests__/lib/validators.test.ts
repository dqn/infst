import { describe, it, expect } from "vitest";

import { validateLoginInput, validateRegisterInput } from "../../lib/validators";

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
