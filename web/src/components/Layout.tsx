import type { Child, FC } from "hono/jsx";

interface LayoutProps {
  title?: string | undefined;
  user?: { username: string | null } | null | undefined;
  children: Child;
}

export const Layout: FC<LayoutProps> = ({ title, user, children }) => {
  const pageTitle = title ? `${title} - infst` : "infst";

  return (
    <html lang="ja">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta name="theme-color" content="#111111" />
        <meta name="description" content="IIDX INFINITAS Score Tracker" />
        <link rel="manifest" href="/manifest.webmanifest" />
        <link rel="apple-touch-icon" href="/icons/apple-touch-icon.png" />
        <meta name="apple-mobile-web-app-capable" content="yes" />
        <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent" />
        <title>{pageTitle}</title>
        <link rel="stylesheet" href="/styles.css" />
      </head>
      <body>
        <nav>
          <div class="nav-inner">
            <a class="brand" href="/">infst</a>
            <div class="links">
              <a href="/guide">Guide</a>
              {user ? (
                <>
                  {user.username ? (
                    <a href={`/${user.username}`}>{user.username}</a>
                  ) : null}
                  <a href="/settings">Settings</a>
                  <form method="post" action="/auth/logout" style="display:inline">
                    <button type="submit" class="secondary" style="padding:6px 12px;font-size:0.85rem">
                      Logout
                    </button>
                  </form>
                </>
              ) : (
                <a href="/login">Login</a>
              )}
            </div>
          </div>
        </nav>
        <div class="container">
          {children}
        </div>
        <script src="/register-sw.js"></script>
      </body>
    </html>
  );
};
