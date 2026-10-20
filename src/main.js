import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { generate, validate } from './generate.js';
import { loadTemplates } from './templates.js';
import { setupDelete } from './delete.js';

const $ = (id) => {
  return document.getElementById(id);
};

const native = isTauri();
let templates;
let context;
let customPath = '';
let busy = false;
const notes = {
  setup: 'UGX’s starter level, ready to open in Radiant.',
  blank: 'An empty map. Add your level and entities in Radiant.',
  custom:
    'Your selected .map is copied unchanged. Starter scripts use the new map name.',
};

const options = () => {
  return {
    name: $('name').value,
    display: $('display').value,
    source: document.querySelector('[name=source]:checked').value,
  };
};

function message(text, kind = '') {
  $('message').textContent = text;
  $('message').className = kind;
}

// Preview paths only; the custom map's contents are read when Generate is pressed.
function refresh() {
  const opts = options();
  $('name-clear').hidden = opts.name.length === 0;
  $('display-clear').hidden = opts.display.length === 0;
  $('custom-row').hidden = opts.source !== 'custom';
  $('source-note').textContent = notes[opts.source];
  $('generate').disabled =
    busy || !templates || !native || Boolean(context?.error);
  if (!templates) {
    return;
  }

  let preview;
  try {
    validate(opts);
    preview = opts;
  } catch {
    preview = { ...opts, name: 'mapname', display: '' };
  }

  const entries = generate(preview, templates, new Uint8Array());
  $('file-list').replaceChildren(
    ...entries.map(({ path }) => {
      const item = document.createElement('li');
      item.textContent = path;
      return item;
    }),
  );
}

$('form').addEventListener('input', refresh);

// Clearing a field follows the same preview update as typing, then restores focus.
for (const id of ['name', 'display']) {
  $(`${id}-clear`).addEventListener('click', () => {
    const input = $(id);
    input.value = '';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.focus();
  });
}

$('browse').addEventListener('click', async () => {
  try {
    const path = await open({
      defaultPath: context.map_source,
      multiple: false,
      directory: false,
      filters: [{ name: 'Radiant map', extensions: ['map'] }],
    });
    if (typeof path === 'string') {
      customPath = path;
      $('custom-path').value = path;
    }
  } catch (error) {
    message(String(error), 'error');
  }
});

// Default every conflict to Skip. Closing the dialog cancels the whole operation.
function review(paths) {
  return new Promise((resolve) => {
    const dialog = $('conflicts');
    $('bulk').value = 'skip';
    $('conflict-title').textContent =
      `${paths.length} ${paths.length === 1 ? 'file already exists' : 'files already exist'}`;
    const selects = paths.map((path) => {
      const row = document.createElement('label');
      row.className = 'conflict';
      const label = document.createElement('span');
      label.textContent = path;
      const select = document.createElement('select');
      select.setAttribute('aria-label', `Action for ${path}`);
      for (const [value, text] of [
        ['skip', 'Skip'],
        ['replace', 'Replace'],
        ['backup', 'Back Up + Replace'],
      ]) {
        select.add(new Option(text, value));
      }
      row.append(label, select);
      return { path, row, select };
    });
    $('conflict-list').replaceChildren(
      ...selects.map((item) => {
        return item.row;
      }),
    );
    $('bulk').onchange = () => {
      for (const { select } of selects) {
        select.value = $('bulk').value;
      }
    };
    $('cancel').onclick = () => {
      return dialog.close('cancel');
    };
    $('confirm').onclick = () => {
      return dialog.close('continue');
    };
    dialog.addEventListener(
      'close',
      () => {
        return resolve(
          dialog.returnValue === 'continue'
            ? Object.fromEntries(
                selects.map(({ path, select }) => {
                  return [path, select.value];
                }),
              )
            : null,
        );
      },
      { once: true },
    );
    dialog.returnValue = 'cancel';
    dialog.showModal();
    $('cancel').focus();
  });
}

$('form').addEventListener('submit', async (event) => {
  event.preventDefault();
  if (busy || !native || context?.error) {
    return;
  }
  busy = true;
  $('inputs').disabled = true;
  refresh();
  $('status').textContent = 'Preparing files…';
  message('');
  try {
    const opts = options();
    validate(opts);
    if (opts.source === 'custom' && !customPath) {
      throw new Error('Choose a custom .map file.');
    }

    const custom =
      opts.source === 'custom'
        ? new Uint8Array(await invoke('read_custom', { path: customPath }))
        : undefined;
    const entries = generate(opts, templates, custom).map(({ path, bytes }) => {
      return { path, bytes: Array.from(bytes) };
    });

    // Prepare checks destinations without writing. Apply runs only after review.
    const conflicts = await invoke('prepare', { name: opts.name, entries });
    const choices = conflicts.length ? await review(conflicts) : {};
    if (choices === null) {
      message('Cancelled. No files were changed.');
      $('status').textContent = 'Cancelled';
      return;
    }
    $('status').textContent = 'Writing files…';
    const result = await invoke('apply', { choices });
    message(
      `${result.written} files written${result.skipped ? ` · ${result.skipped} skipped` : ''}${result.backups.length ? ` · ${result.backups.length} backed up` : ''}.\n${result.written ? `Open map_source/${opts.name}.map in Radiant.` : 'Existing files were left untouched.'}`,
      'success',
    );
    $('status').textContent = 'Done';
  } catch (error) {
    message(error instanceof Error ? error.message : String(error), 'error');
    $('status').textContent = 'Could not generate';
  } finally {
    busy = false;
    $('inputs').disabled = false;
    refresh();
  }
});

$('about-button').onclick = () => {
  $('about').showModal();
  $('repo-url').focus();
  $('repo-url').select();
};
$('about-close').onclick = () => {
  return $('about').close();
};
$('repo-url').onclick = (event) => {
  return event.target.select();
};
$('startup-error').addEventListener('cancel', (event) => {
  return event.preventDefault();
});
$('quit').onclick = () => {
  return invoke('quit');
};

function startupError(text) {
  $('app').hidden = true;
  $('footer').hidden = true;
  $('startup-message').textContent = text;
  $('startup-error').showModal();
}

// Show the form only after startup checks and template loading succeed.
async function start() {
  if (native) {
    context = await invoke('context');
    if (context.error) {
      startupError(context.error);
      return;
    }
    $('root').textContent = context.root;
    $('mode').textContent = context.development ? 'Development' : 'Production';
    if (context.development) {
      $('root-note').textContent =
        'Development output folder. Your game installation is not selected automatically.';
    }
    $('status').textContent = 'Ready';
  } else {
    $('root').textContent = 'Desktop app required';
    $('root-note').textContent =
      'Browser preview only. Run npm run dev to open the desktop app.';
    $('browse').disabled = true;
    $('status').textContent = 'UI preview';
    $('mode').textContent = 'Browser preview';
  }
  templates = await loadTemplates();
  setupDelete(native, context);
  refresh();
  $('app').hidden = false;
  $('footer').hidden = false;
}
start().catch((error) => {
  return startupError(String(error));
});
