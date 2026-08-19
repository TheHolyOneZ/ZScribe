#!/usr/bin/env node


import { execSync, spawn } from "node:child_process";
import { createRequire } from "node:module";

import { candidates, findLibclang, holdsLibclang } from "./libclang.mjs";
import {
  candidates as vulkanCandidates,
  findVulkanSdk,
  holdsVulkanSdk,
} from "./vulkan.mjs";


function tauriEntryPoint() {
  try {
    return createRequire(import.meta.url).resolve("@tauri-apps/cli/tauri.js");
  } catch {
    return null;
  }
}

function onPath(command) {
  try {


    execSync(`${command} --version`, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}


function withoutDefaultFeatures(args) {
  if (args.includes("--no-default-features")) return args;
  return args.includes("--")
    ? [...args, "--no-default-features"]
    : [...args, "--", "--no-default-features"];
}


function withPlatformGpu(args) {
  if (process.platform !== "darwin") return args;


  if (args.some((arg) => arg === "--features" || arg.startsWith("--features="))) return args;
  if (args.includes("--no-default-features")) return args;

  const flags = ["--no-default-features", "--features", "metal"];
  return args.includes("--") ? [...args, ...flags] : [...args, "--", ...flags];
}


function vulkanSdkUsable(sdk) {
  if (process.platform !== "win32") return true;
  return holdsVulkanSdk(sdk);
}

const args = process.argv.slice(2);

if (args.length === 0) {
  console.error("Usage: node scripts/run-tauri.mjs <dev|build|…> [args…]");
  process.exit(2);
}

const env = { ...process.env };

if (process.platform === "linux") {
  env.NO_STRIP ??= "true";
}


const compiles = args[0] === "dev" || args[0] === "build";


if (compiles && /[()[\]\s]/.test(process.cwd())) {
  console.warn(
    [
      "",
      `  Warning: the project path contains a space or bracket:`,
      `      ${process.cwd()}`,
      "  CMake handles these badly. If the whisper.cpp build fails for a reason",
      "  that makes no sense, rename the folder and try again.",
      "",
    ].join("\n"),
  );
}


if (compiles && !onPath("cmake")) {
  console.error(
    [
      "",
      "  cmake was not found, and whisper.cpp is built with it.",
      "",
      process.platform === "win32"
        ? "      winget install --id Kitware.CMake"
        : "      Debian/Ubuntu: apt install cmake     Arch: pacman -S cmake",
      "",
      "  Open a new terminal afterwards — the installer adds it to PATH, and an",
      "  already-running shell keeps the old one.",
      "",
    ].join("\n"),
  );
  process.exit(1);
}

if (process.platform === "win32" && compiles && !holdsLibclang(env.LIBCLANG_PATH)) {
  const found = findLibclang();

  if (!found) {
    console.error(
      [
        "",
        "  libclang.dll was not found, and whisper-rs-sys cannot build without it.",
        "  It generates its bindings with bindgen, so this is needed even for a",
        "  CPU-only build.",
        "",
        "  Install LLVM, then run this again — no environment variable needed:",
        "",
        "      winget install --id LLVM.LLVM",
        "",
        "  Run `pnpm check:env` for a fuller report. Looked in:",
        ...candidates().map((dir) => `      ${dir}`),
        "",
      ].join("\n"),
    );
    process.exit(1);
  }

  if (env.LIBCLANG_PATH) {
    console.log(`  Ignoring LIBCLANG_PATH=${env.LIBCLANG_PATH} — no libclang.dll there`);
  }
  console.log(`  Using libclang from ${found}`);
  env.LIBCLANG_PATH = found;
}


if (process.platform === "win32" && compiles && !holdsVulkanSdk(env.VULKAN_SDK)) {
  const sdk = findVulkanSdk();
  if (sdk) {
    if (env.VULKAN_SDK) {
      console.log(`  Ignoring VULKAN_SDK=${env.VULKAN_SDK} — no SDK there`);
    }
    console.log(`  Using the Vulkan SDK from ${sdk}`);
    env.VULKAN_SDK = sdk;
  }
}


let effective = compiles ? withPlatformGpu(args) : args;

if (compiles && effective !== args) {
  console.log("\n  Building with Metal, which every Mac has. No Vulkan SDK needed.\n");
}

if (compiles && !vulkanSdkUsable(env.VULKAN_SDK)) {
  effective = withoutDefaultFeatures(effective);

  if (effective !== args) {
    console.log(
      [
        "",
        "  No Vulkan SDK found, so this build will transcribe on the processor.",
        "  Everything else works the same; long recordings just take longer.",
        "",
        "  For the graphics card, install the SDK once — the build finds it on",
        "  its own afterwards, no VULKAN_SDK variable to set:",
        "      winget install --id KhronosGroup.VulkanSDK",
        "",
        "  Looked in:",
        ...vulkanCandidates().map((dir) => `      ${dir}`),
        "",
      ].join("\n"),
    );
  }
}

const entry = tauriEntryPoint();

if (!entry) {
  console.error("\n  The Tauri CLI was not found. Run `pnpm install` first.\n");
  process.exit(1);
}

const tauri = spawn(process.execPath, [entry, ...effective], {
  stdio: "inherit",
  env,
});

tauri.on("close", (code) => process.exit(code ?? 1));
