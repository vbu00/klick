// Собрать установщик kl!ck (setup/) и положить в dist/ под именем релиза.
// Сначала должен быть собран сам kl!ck: npm run build — его NSIS-установщик
// вшивается внутрь (setup/src-tauri/build.rs).
//   npm run build:setup   → dist/klick-<версия>-x64-setup.exe
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const version = JSON.parse(fs.readFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), 'utf8')).version;
const cli = path.join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
execFileSync(process.execPath, [cli, 'build', '--no-bundle'], { cwd: path.join(root, 'setup'), stdio: 'inherit' });
const built = path.join(root, 'setup', 'src-tauri', 'target', 'release', 'klick-setup.exe');
fs.mkdirSync(path.join(root, 'dist'), { recursive: true });
const out = path.join(root, 'dist', `klick-${version}-x64-setup.exe`);
fs.copyFileSync(built, out);
console.log(`\nготово: ${out} (${(fs.statSync(out).size / 1e6).toFixed(1)} МБ)`);
