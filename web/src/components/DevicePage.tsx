import type { FC } from "hono/jsx";
import { Layout } from "./Layout";

interface DevicePageProps {
  userCode: string;
  error?: string;
  success?: boolean;
}

export const DevicePage: FC<DevicePageProps> = ({
  userCode,
  error,
  success,
}) => {
  return (
    <Layout title="Device Authorization">
      <div style="max-width:400px;margin:48px auto;">
        <h2 style="margin-bottom:24px;">Device Authorization</h2>
        <div class="card">
          {error ? <p class="error">{error}</p> : null}
          {success ? (
            <div>
              <p class="success">Device authorized successfully!</p>
              <p style="color:#999;">
                You can close this page and return to infst.
              </p>
            </div>
          ) : (
            <>
              <p style="color:#999;margin-bottom:16px;">
                Enter the code displayed on your device to authorize it.
              </p>
              <form method="post" action="/auth/device/confirm">
                <div style="margin-bottom:12px;">
                  <input
                    type="text"
                    name="user_code"
                    value={userCode}
                    placeholder="XXXX-XXXX"
                    required
                    style="width:100%;font-size:1.5rem;text-align:center;letter-spacing:4px;padding:14px 20px;"
                  />
                </div>
                <button type="submit" style="width:100%;">Authorize</button>
              </form>
            </>
          )}
        </div>
      </div>
    </Layout>
  );
};
