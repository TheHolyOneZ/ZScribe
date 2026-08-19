

import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";


export function holdsVulkanSdk(dir) {
  return (
    Boolean(dir) &&
    existsSync(join(dir, "Include", "vulkan", "vulkan.h")) &&
    existsSync(join(dir, "Lib", "vulkan-1.lib"))
  );
}

function versionDirsDescending(root) {
  try {
    return readdirSync(root, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)


      .sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
  } catch {
    return [];
  }
}


export function candidates() {
  const found = [];
  const add = (path) => {
    if (path && !found.includes(path)) found.push(path);
  };


  add(process.env.VULKAN_SDK);
  add(process.env.VK_SDK_PATH);


  const systemDrive = process.env.SystemDrive ?? "C:";
  for (const root of [join(systemDrive, "\\", "VulkanSDK"), "C:\\VulkanSDK"]) {
    for (const version of versionDirsDescending(root)) {
      add(join(root, version));
    }
  }

  return found;
}


export function findVulkanSdk() {
  for (const dir of candidates()) {
    if (holdsVulkanSdk(dir)) return dir;
  }
  return null;
}
