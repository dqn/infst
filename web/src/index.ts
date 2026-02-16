import { Hono } from "hono";
import { csrf } from "hono/csrf";

import type { AppEnv } from "./lib/types";
import { dbMiddleware } from "./middleware/db";
import { rateLimit } from "./middleware/rate-limit";
import { setupErrorHandler } from "./middleware/error-handler";
import { authRoutes } from "./routes/auth";
import { apiRoutes } from "./routes/api";
import { pageRoutes } from "./routes/pages";
import {
  cleanupExpiredDeviceCodes,
  cleanupRateLimits,
} from "./lib/cleanup";
import {
  RATE_LIMIT_LOGIN_MAX,
  RATE_LIMIT_LOGIN_WINDOW_SECONDS,
  RATE_LIMIT_REGISTER_MAX,
  RATE_LIMIT_REGISTER_WINDOW_SECONDS,
  RATE_LIMIT_DEVICE_CODE_MAX,
  RATE_LIMIT_DEVICE_CODE_WINDOW_SECONDS,
} from "./lib/constants";

const app = new Hono<AppEnv>();

// Global error handler
setupErrorHandler(app);

// DB middleware for all routes
app.use("*", dbMiddleware);

// CSRF protection for browser form submissions
app.use("/auth/login", csrf());
app.use("/auth/register", csrf());
app.use("/auth/device/confirm", csrf());

// CSRF protection for session-authenticated API endpoints
app.use("/api/users/me", csrf());
app.use("/api/users/me/*", csrf());

// Rate limiting
app.use(
  "/auth/login",
  rateLimit({
    max: RATE_LIMIT_LOGIN_MAX,
    windowSeconds: RATE_LIMIT_LOGIN_WINDOW_SECONDS,
  }),
);
app.use(
  "/auth/register",
  rateLimit({
    max: RATE_LIMIT_REGISTER_MAX,
    windowSeconds: RATE_LIMIT_REGISTER_WINDOW_SECONDS,
  }),
);
app.use(
  "/auth/device/code",
  rateLimit({
    max: RATE_LIMIT_DEVICE_CODE_MAX,
    windowSeconds: RATE_LIMIT_DEVICE_CODE_WINDOW_SECONDS,
  }),
);

// Routes
app.route("/auth", authRoutes);
app.route("/api", apiRoutes);
app.route("/", pageRoutes);

export default {
  fetch: app.fetch,
  // Cron Trigger: daily cleanup at 3:00 UTC
  async scheduled(_event: ScheduledEvent, env: { DB: D1Database }) {
    await cleanupExpiredDeviceCodes(env.DB);
    await cleanupRateLimits(env.DB);
  },
};
