import { open, save, confirm } from '@tauri-apps/plugin-dialog';
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import { openUrl } from '@tauri-apps/plugin-opener';
import { onBackButtonPress, exit } from '@tauri-apps/api/app';

export const confirmAction = message => confirm(message, { title: 'BookLib', kind: 'warning' });

export async function saveBackup(data, filename) {
  const path = await save({ defaultPath: filename, filters: [{ name: 'BookLib backup', extensions: ['json'] }] });
  if (!path) return false;
  await writeTextFile(path, JSON.stringify(data, null, 2));
  return true;
}

export async function loadBackup() {
  const path = await open({ multiple: false, directory: false, filters: [{ name: 'BookLib backup', extensions: ['json'] }] });
  return path ? JSON.parse(await readTextFile(path)) : null;
}

export async function openExternal(url) {
  const target = new URL(url);
  if (target.protocol !== 'https:' || !['www.amazon.es', 'openlibrary.org'].includes(target.hostname)) {
    throw new Error('Unsupported external link');
  }
  await openUrl(target.href);
}

export function installExternalLinks(onError) {
  document.addEventListener('click', event => {
    const link = event.target.closest('a[href^="https:"]');
    if (!link) return;
    event.preventDefault();
    openExternal(link.href).catch(onError);
  });
  document.addEventListener('submit', event => {
    const form = event.target;
    if (!form.matches('form[action="https://www.amazon.es/s"]')) return;
    event.preventDefault();
    const url = new URL(form.action);
    url.search = new URLSearchParams(new FormData(form)).toString();
    openExternal(url.href).catch(onError);
  });
}

export async function installBackButton(onError) {
  await onBackButtonPress(() => {
    const dialogs = [...document.querySelectorAll('dialog[open]')];
    if (dialogs.length) {
      dialogs.at(-1).close();
    } else if (!document.querySelector('#edition-area').hidden) {
      document.querySelector('#close-editions').click();
    } else if (location.hash && location.hash !== '#inicio') {
      location.hash = '#inicio';
    } else {
      exit(0).catch(onError);
    }
  });
}
