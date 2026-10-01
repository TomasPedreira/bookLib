import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(import.meta.url);
const cli = join(dirname(require.resolve('@tauri-apps/cli/package.json')), 'tauri.js');
const result = spawnSync(process.execPath, [cli, 'icon', 'src-tauri/icons/icon-manifest.json'], {
  cwd: root,
  stdio: 'inherit',
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

// Tauri writes Android icons into the generated project when it exists.
// Keep the reusable icon assets in sync with those packaged by Android.
const android = join(root, 'src-tauri/gen/android/app/src/main/res');
const assets = join(root, 'src-tauri/icons/android');
if (existsSync(android)) {
  for (const directory of readdirSync(android).filter(name => name.startsWith('mipmap-'))) {
    const destination = join(assets, directory);
    mkdirSync(destination, { recursive: true });
    for (const name of readdirSync(join(android, directory)).filter(name => name.startsWith('ic_launcher'))) {
      copyFileSync(join(android, directory, name), join(destination, name));
    }
  }
}
mkdirSync(join(assets, 'values'), { recursive: true });
writeFileSync(join(assets, 'values/ic_launcher_background.xml'),
  '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n  <color name="ic_launcher_background">#f4f1e8</color>\n</resources>\n');
