import { cpSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";

execFileSync(process.execPath, ["./node_modules/vite/bin/vite.js", "build", "--config", "vite.standalone.config.ts"], { stdio: "inherit" });
cpSync("dist-standalone/send/index.html", "qrbeam-send.html");
rmSync("dist-standalone", { recursive: true, force: true });
