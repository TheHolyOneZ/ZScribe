

import { existsSync, readdirSync } from "node:fs";
import { delimiter, join } from "node:path";

const DLL = "libclang.dll";

function directoriesIn(path) {
  try {
    return readdirSync(path, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name);
  } catch {
    return [];
  }
}


export function candidates() {
  const programFiles = process.env.ProgramFiles ?? "C:\\Program Files";
  const programFilesX86 = process.env["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)";
  const localAppData = process.env.LOCALAPPDATA;
  const userProfile = process.env.USERPROFILE;
  const programData = process.env.ProgramData ?? "C:\\ProgramData";

  const found = [];
  const add = (path) => {
    if (path && !found.includes(path)) found.push(path);
  };


  add(join(programFiles, "LLVM", "bin"));
  add(join(programFilesX86, "LLVM", "bin"));
  if (localAppData) add(join(localAppData, "Programs", "LLVM", "bin"));


  for (const name of directoriesIn(join(programData, "chocolatey", "lib"))) {
    if (name.toLowerCase().startsWith("llvm")) {
      add(join(programData, "chocolatey", "lib", name, "tools", "LLVM", "bin"));
    }
  }
  if (userProfile) add(join(userProfile, "scoop", "apps", "llvm", "current", "bin"));


  add("C:\\msys64\\mingw64\\bin");
  add("C:\\msys64\\clang64\\bin");


  for (const root of [programFiles, programFilesX86]) {
    const vs = join(root, "Microsoft Visual Studio");
    for (const year of directoriesIn(vs)) {
      for (const edition of directoriesIn(join(vs, year))) {
        add(join(vs, year, edition, "VC", "Tools", "Llvm", "x64", "bin"));
        add(join(vs, year, edition, "VC", "Tools", "Llvm", "bin"));
      }
    }
  }


  for (const entry of (process.env.PATH ?? "").split(delimiter)) {
    if (entry.trim()) add(entry.trim());
  }

  return found;
}


export function findLibclang() {
  for (const dir of candidates()) {
    if (existsSync(join(dir, DLL))) return dir;
  }
  return null;
}


export function holdsLibclang(dir) {
  return Boolean(dir) && existsSync(join(dir, DLL));
}
