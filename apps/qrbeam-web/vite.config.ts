import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  base: "/qrbeam/",
  plugins: [VitePWA({ registerType: "autoUpdate", manifest: { name: "QRBeam", short_name: "QRBeam", display: "standalone", start_url: "/qrbeam/", theme_color: "#090909", background_color: "#090909" } })],
  build: { rollupOptions: { input: { home: "index.html", send: "send/index.html", receive: "receive/index.html" } } }
});
