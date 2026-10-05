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
// The foreground bitmap fills its 108dp canvas. Keep the book inside the
// adaptive icon safe area so launcher masks cannot crop its cover or pages.
const adaptiveIcon = `<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
  <background android:drawable="@mipmap/ic_launcher_background"/>
  <foreground>
    <inset android:drawable="@mipmap/ic_launcher_foreground"
      android:insetLeft="18%" android:insetTop="18%"
      android:insetRight="18%" android:insetBottom="18%"/>
  </foreground>
</adaptive-icon>
`;
for (const resources of existsSync(android) ? [android, assets] : [assets]) {
  const adaptiveDirectory = join(resources, 'mipmap-anydpi-v26');
  mkdirSync(adaptiveDirectory, { recursive: true });
  for (const name of ['ic_launcher.xml', 'ic_launcher_round.xml']) {
    writeFileSync(join(adaptiveDirectory, name), adaptiveIcon);
  }
}
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
  '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n  <color name="ic_launcher_background">#ffffff</color>\n</resources>\n');
