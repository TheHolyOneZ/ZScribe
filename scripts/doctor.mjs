#!/usr/bin/env node


import { execSync } from "node:child_process";
import { existsSync } from "node:fs";

import { candidates, findLibclang, holdsLibclang } from "./libclang.mjs";
import { candidates as vulkanCandidates, findVulkanSdk } from "./vulkan.mjs";


const ok = "  ok   ";
const no = "MISSING";
const warn = " check ";

function line(status, label, detail = "") {
  console.log(`[${status}] ${label.padEnd(22)} ${detail}`);
}


function version(command) {
  try {
    return execSync(`${command} --version`, {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    })
      .trim()
      .split("\n")[0];
  } catch {
    return null;
  }
}

console.log(`\nZScribe — environment report\n`);
console.log(`  platform   ${process.platform} ${process.arch}`);
console.log(`  node       ${process.version}`);
console.log(`  cwd        ${process.cwd()}\n`);


if (/[()\s]/.test(process.cwd())) {
  line(warn, "project path", "contains a space or bracket — CMake handles these badly");
} else {
  line(ok, "project path", "no spaces or brackets");
}


const windows = process.platform === "win32";
const remedies = {
  cargo: windows ? "winget install --id Rustlang.Rustup" : "install rustup",
  rustc: windows ? "winget install --id Rustlang.Rustup" : "install rustup",
  pnpm: "npm install -g pnpm",
  cmake: windows ? "winget install --id Kitware.CMake" : "apt install cmake / pacman -S cmake",
};

for (const command of ["cargo", "rustc", "pnpm", "cmake"]) {
  const found = version(command);
  line(found ? ok : no, command, found ?? `not on PATH — ${remedies[command]}`);
}

if (process.platform === "win32") {
  console.log();

  const configured = process.env.LIBCLANG_PATH;
  if (configured) {
    line(
      holdsLibclang(configured) ? ok : warn,
      "LIBCLANG_PATH",
      holdsLibclang(configured) ? configured : `${configured} — no libclang.dll there`,
    );
  } else {
    line(ok, "LIBCLANG_PATH", "not set (it does not need to be)");
  }

  const found = findLibclang();
  line(found ? ok : no, "libclang.dll", found ?? "not found in any known location");

  if (!found) {
    console.log("\n  Looked in:");
    for (const dir of candidates()) {
      console.log(`      ${existsSync(dir) ? " " : "-"} ${dir}`);
    }
    console.log("\n  ( - means the directory does not exist )");
    console.log("\n  Fix:  winget install --id LLVM.LLVM\n");
  }


  const sdk = findVulkanSdk();
  line(
    sdk ? ok : warn,
    "Vulkan SDK",
    sdk
      ? `${sdk} — the graphics-card build will use it`
      : "not found — the build falls back to the processor (winget install --id KhronosGroup.VulkanSDK)",
  );

  if (!sdk) {
    console.log("\n  Looked in:");
    for (const dir of vulkanCandidates()) {
      console.log(`      ${existsSync(dir) ? " " : "-"} ${dir}`);
    }
    if (vulkanCandidates().length === 0) {
      console.log("      (nowhere — no VULKAN_SDK set and no C:\\VulkanSDK)");
    }
  }
}


if (process.platform === "darwin") {
  console.log();
  line(ok, "Metal", "part of macOS — the graphics card needs no SDK here");
}

console.log(`\n  Build with:`);
console.log(`      pnpm app:dev:cpu      transcription on the processor, needs only LLVM`);
console.log(
  process.platform === "darwin"
    ? `      pnpm app:dev          uses the graphics card through Metal\n`
    : `      pnpm app:dev          uses the graphics card, also needs the Vulkan SDK\n`,
);
