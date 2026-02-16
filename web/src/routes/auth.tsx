import { Hono } from "hono";
import { eq } from "drizzle-orm";

import type { AppEnv } from "../lib/types";
import { generateToken, generateUserCode } from "../lib/token";
import { hashPassword, verifyPassword } from "../lib/password";
import { users, deviceCodes } from "../db/schema";
import {
  sessionAuth,
  createSessionCookie,
  setSessionCookie,
} from "../middleware/session";
import { DevicePage } from "../components/DevicePage";
import { validateLoginInput, validateRegisterInput } from "../lib/validators";
import {
  DEVICE_CODE_EXPIRY_MS,
  POLLING_INTERVAL_SECONDS,
  RESERVED_USERNAMES,
} from "../lib/constants";
import { maybeCleanupDeviceCodes } from "../lib/cleanup";

export const authRoutes = new Hono<AppEnv>();

// POST /auth/login - Email + password login
authRoutes.post("/login", async (c) => {
  const body = await c.req.parseBody();
  const email = body["email"];
  const password = body["password"];

  const validation = validateLoginInput(email, password);
  if (!validation.valid) {
    const { LoginPage } = await import("../components/LoginPage");
    return c.html(<LoginPage error={validation.error!} />);
  }

  const db = c.get("db");
  const result = await db
    .select()
    .from(users)
    .where(eq(users.email, email as string))
    .limit(1);

  const user = result[0];

  if (!user) {
    // Timing attack mitigation: compute a dummy hash
    await hashPassword(password as string);
    const { LoginPage } = await import("../components/LoginPage");
    return c.html(<LoginPage error="Invalid email or password" values={{ email: email as string }} />);
  }

  const valid = await verifyPassword(password as string, user.passwordHash);
  if (!valid) {
    const { LoginPage } = await import("../components/LoginPage");
    return c.html(<LoginPage error="Invalid email or password" values={{ email: email as string }} />);
  }

  const sessionToken = await createSessionCookie(user.id, c.env.JWT_SECRET);
  setSessionCookie(c, sessionToken);

  return c.redirect("/");
});

// POST /auth/register - Email + password + username registration
authRoutes.post("/register", async (c) => {
  const body = await c.req.parseBody();
  const email = body["email"];
  const password = body["password"];
  const username = body["username"];

  const validation = validateRegisterInput(email, password, username);
  if (!validation.valid) {
    const { RegisterPage } = await import("../components/RegisterPage");
    return c.html(
      <RegisterPage
        error={validation.error!}
        values={{
          email: typeof email === "string" ? email : "",
          username: typeof username === "string" ? username : "",
        }}
      />,
    );
  }

  const values = { email: email as string, username: username as string };
  const trimmed = (username as string).trim().toLowerCase();

  if (RESERVED_USERNAMES.includes(trimmed)) {
    const { RegisterPage } = await import("../components/RegisterPage");
    return c.html(
      <RegisterPage error="This username is not available" values={values} />,
    );
  }

  const db = c.get("db");

  // Check email uniqueness
  const existingEmail = await db
    .select()
    .from(users)
    .where(eq(users.email, email as string))
    .limit(1);

  if (existingEmail.length > 0) {
    const { RegisterPage } = await import("../components/RegisterPage");
    return c.html(
      <RegisterPage error="This email is already registered" values={values} />,
    );
  }

  // Check username uniqueness
  const existingUsername = await db
    .select()
    .from(users)
    .where(eq(users.username, trimmed))
    .limit(1);

  if (existingUsername.length > 0) {
    const { RegisterPage } = await import("../components/RegisterPage");
    return c.html(
      <RegisterPage error="Username already taken" values={values} />,
    );
  }

  const passwordHash = await hashPassword(password as string);
  const apiToken = generateToken();
  const now = new Date().toISOString();

  const inserted = await db
    .insert(users)
    .values({
      email: email as string,
      username: trimmed,
      passwordHash,
      apiToken,
      apiTokenCreatedAt: now,
    })
    .returning();

  const user = inserted[0];
  if (!user) {
    const { RegisterPage } = await import("../components/RegisterPage");
    return c.html(
      <RegisterPage error="Failed to create account" values={values} />,
    );
  }

  // Auto-login after registration
  const sessionToken = await createSessionCookie(user.id, c.env.JWT_SECRET);
  setSessionCookie(c, sessionToken);

  return c.redirect("/");
});

// POST /auth/device/code - Generate device code + user code
authRoutes.post("/device/code", async (c) => {
  const deviceCode = generateToken();
  const userCode = generateUserCode();
  const expiresAt = new Date(Date.now() + DEVICE_CODE_EXPIRY_MS).toISOString();

  const db = c.get("db");
  await db.insert(deviceCodes).values({
    deviceCode,
    userCode,
    expiresAt,
  });

  return c.json({
    device_code: deviceCode,
    user_code: userCode,
    expires_in: DEVICE_CODE_EXPIRY_MS / 1000,
    interval: POLLING_INTERVAL_SECONDS,
    // Bug-3 fix: CLI expects verification_url, not verification_uri
    verification_url: `${c.env.APP_URL}/auth/device`,
  });
});

// POST /auth/device/token - Poll for device authorization
authRoutes.post("/device/token", async (c) => {
  const body = await c.req.json<{ device_code?: string }>();
  if (!body.device_code) {
    return c.json({ error: "device_code is required" }, 400);
  }

  const db = c.get("db");
  const result = await db
    .select()
    .from(deviceCodes)
    .where(eq(deviceCodes.deviceCode, body.device_code))
    .limit(1);

  const code = result[0];
  if (!code) {
    return c.json({ error: "invalid_device_code" }, 400);
  }

  if (new Date(code.expiresAt) < new Date()) {
    return c.json({ error: "expired_token" }, 400);
  }

  // Bug-1 fix: Return format matching CLI's TokenResponse { status, token }
  if (code.apiToken) {
    // Probabilistic cleanup of expired device codes
    await maybeCleanupDeviceCodes(c.env.DB);

    return c.json({ status: "approved", token: code.apiToken });
  }

  return c.json({ status: "pending" }, 428);
});

// GET /auth/device - Device confirmation page (requires session)
authRoutes.get("/device", sessionAuth, async (c) => {
  const userCode = c.req.query("code");
  return c.html(<DevicePage userCode={userCode ?? ""} />);
});

// POST /auth/device/confirm - Confirm device authorization
authRoutes.post("/device/confirm", sessionAuth, async (c) => {
  const body = await c.req.parseBody();
  const userCode = body["user_code"];
  if (typeof userCode !== "string" || !userCode.trim()) {
    return c.html(<DevicePage userCode="" error="User code is required" />);
  }

  const db = c.get("db");
  const result = await db
    .select()
    .from(deviceCodes)
    .where(eq(deviceCodes.userCode, userCode.trim().toUpperCase()))
    .limit(1);

  const code = result[0];
  if (!code) {
    return c.html(
      <DevicePage userCode={userCode} error="Invalid code" />,
    );
  }

  if (new Date(code.expiresAt) < new Date()) {
    return c.html(
      <DevicePage userCode={userCode} error="Code expired" />,
    );
  }

  if (code.apiToken) {
    return c.html(
      <DevicePage userCode={userCode} error="Code already used" />,
    );
  }

  const user = c.get("user");

  // Ensure user has an API token
  let apiToken = user!.apiToken;
  const now = new Date().toISOString();
  if (!apiToken) {
    apiToken = generateToken();
    await db
      .update(users)
      .set({ apiToken, apiTokenCreatedAt: now })
      .where(eq(users.id, user!.id));
  }

  // Link device code to user
  await db
    .update(deviceCodes)
    .set({ userId: user!.id, apiToken })
    .where(eq(deviceCodes.deviceCode, code.deviceCode));

  return c.html(
    <DevicePage userCode={userCode} success={true} />,
  );
});

// POST /auth/logout
authRoutes.post("/logout", (c) => {
  c.header(
    "Set-Cookie",
    "session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0",
  );
  return c.redirect("/login");
});
