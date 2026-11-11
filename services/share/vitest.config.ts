import { cloudflareTest } from "@cloudflare/vitest-pool-workers";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [
    cloudflareTest({
      wrangler: { configPath: "./wrangler.jsonc" },
      miniflare: {
        bindings: {
          POW_DIFFICULTY: "8",
          DAILY_SHARES: "5",
          STORAGE_STOP_BYTES: String(300 * 1024 * 1024),
          CHALLENGE_KEY: "test-challenge-key-0123456789abcdef",
          INSTALL_KEY: "test-install-key-0123456789abcdefgh",
          UPLOAD_KEY: "test-upload-key-0123456789abcdefghi",
          DELETE_KEY: "test-delete-key-0123456789abcdefghi",
          IP_KEY: "test-ip-key-0123456789abcdefghijklmn",
        },
      },
    }),
  ],
  test: { setupFiles: ["./test/setup.ts"], testTimeout: 60_000 },
});
