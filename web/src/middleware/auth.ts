import { createMiddleware } from "hono/factory";
import { eq } from "drizzle-orm";

import type { AppEnv } from "../lib/types";
import { users } from "../db/schema";
import { API_TOKEN_EXPIRY_DAYS } from "../lib/constants";

// Bearer token authentication middleware for API routes
export const bearerAuth = createMiddleware<AppEnv>(async (c, next) => {
  const authorization = c.req.header("Authorization");
  if (!authorization) {
    return c.json({ error: "Missing Authorization header" }, 401);
  }

  const match = authorization.match(/^Bearer\s+(.+)$/);
  if (!match?.[1]) {
    return c.json({ error: "Invalid Authorization header format" }, 401);
  }

  const token = match[1];
  const db = c.get("db");
  const result = await db
    .select()
    .from(users)
    .where(eq(users.apiToken, token))
    .limit(1);

  const user = result[0];
  if (!user) {
    return c.json({ error: "Invalid API token" }, 401);
  }

  // Check API token expiry (90 days)
  if (!user.apiTokenCreatedAt) {
    return c.json({ error: "token_expired" }, 401);
  }
  const createdAt = new Date(user.apiTokenCreatedAt).getTime();
  const expiryMs = API_TOKEN_EXPIRY_DAYS * 24 * 60 * 60 * 1000;
  if (Date.now() - createdAt > expiryMs) {
    return c.json({ error: "token_expired" }, 401);
  }

  c.set("user", user);
  await next();
});
