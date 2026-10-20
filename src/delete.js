import { invoke } from '@tauri-apps/api/core';

const sections = [
  [
    'Starter files',
    'The 18 paths created by Generate. Includes any edits made to those files.',
  ],
  [
    'Other files in the game folder',
    'Everything else in this mod’s folder, plus recognised build outputs and backups elsewhere.',
  ],
  [
    'Playable copy in AppData',
    'Files inside this map’s Activision/CoDWaW/mods folder.',
  ],
];

export function setupDelete(native, context) {
  const get = (id) => {
    return document.getElementById(id);
  };
  let preview = null;
  let busy = false;
  let checks = [];
  let groupChecks = [];

  function message(text, error = false, success = false) {
    get('delete-message').textContent = text;
    get('delete-message').classList.toggle('error', error);
    get('delete-message').classList.toggle('success', success);
  }

  function updateSelection() {
    const selected = checks.filter(({ input }) => {
      return input.checked;
    });
    get('delete-review').disabled = busy || selected.length === 0;
    get('delete-review').textContent = selected.length
      ? `Review deletion (${selected.length})`
      : 'Review deletion';

    for (const { input, members } of [
      ...groupChecks,
      { input: get('delete-all'), members: checks },
    ]) {
      const count = members.filter((row) => {
        return row.input.checked;
      }).length;
      input.disabled = busy || members.length === 0;
      input.checked = members.length > 0 && count === members.length;
      input.indeterminate = count > 0 && count < members.length;
    }
  }

  function lock(value) {
    busy = value;
    get('delete-name').disabled = value || !native;
    get('delete-clear').disabled = value || !native;
    get('delete-scan').disabled = value || !native;
    get('generate-tab').disabled = value;
    for (const { input } of checks) {
      input.disabled = value;
    }
    updateSelection();
  }

  function render() {
    checks = [];
    groupChecks = [];
    get('delete-sections').replaceChildren();

    for (const [section, [title, description]] of sections.entries()) {
      const rows =
        preview?.items.filter((item) => {
          return item.section === section;
        }) || [];
      const card = document.createElement('section');
      card.className = 'delete-section';
      const heading = document.createElement('div');
      heading.className = 'delete-heading';
      const label = document.createElement('label');
      label.className = 'check-label';
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.setAttribute('aria-label', `Select all ${title.toLowerCase()}`);
      const text = document.createElement('strong');
      text.textContent = `${section + 1}. ${title}`;
      label.append(input, text);
      const count = document.createElement('span');
      count.className = 'hint';
      count.textContent = `${rows.length} ${rows.length === 1 ? 'file' : 'files'}`;
      heading.append(label, count);
      const hint = document.createElement('p');
      hint.className = 'hint';
      hint.textContent = description;
      card.append(heading, hint);

      if (section === 2 && preview) {
        const location = document.createElement('p');
        location.className = 'hint delete-location';
        location.textContent = preview.appdata || 'AppData folder unavailable.';
        card.append(location);
      }

      const members = [];
      if (rows.length) {
        const details = document.createElement('details');
        const summary = document.createElement('summary');
        summary.textContent = 'Choose individual files';
        const list = document.createElement('div');
        list.className = 'delete-file-list';

        for (const item of rows) {
          const row = document.createElement('label');
          row.className = 'delete-file';
          const checkbox = document.createElement('input');
          checkbox.type = 'checkbox';
          checkbox.addEventListener('change', updateSelection);
          const path = document.createElement('span');
          path.textContent = item.path;
          row.append(checkbox, path);
          list.append(row);
          const member = { input: checkbox, item };
          members.push(member);
          checks.push(member);
        }
        details.append(summary, list);
        card.append(details);
      }

      input.addEventListener('change', () => {
        for (const member of members) {
          member.input.checked = input.checked;
        }
        updateSelection();
      });
      groupChecks.push({ input, members });
      get('delete-sections').append(card);
    }
    updateSelection();
  }

  async function loadMaps() {
    if (!native) {
      return;
    }
    const names = await invoke('delete_maps');
    get('delete-maps').replaceChildren(
      ...names.map((name) => {
        return new Option(name, name);
      }),
    );
  }

  function activate(tab) {
    if (busy || get('inputs').disabled) {
      return;
    }
    const deleting = tab === 'delete';
    get('generate-panel').hidden = deleting;
    get('delete-panel').hidden = !deleting;
    for (const name of ['generate', 'delete']) {
      get(`${name}-tab`).setAttribute('aria-selected', String(name === tab));
      get(`${name}-tab`).tabIndex = name === tab ? 0 : -1;
    }
    if (deleting) {
      loadMaps().catch((error) => {
        message(String(error), true);
      });
    }
  }

  for (const tab of ['generate', 'delete']) {
    get(`${tab}-tab`).addEventListener('click', () => {
      activate(tab);
    });
    get(`${tab}-tab`).addEventListener('keydown', (event) => {
      if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
        event.preventDefault();
        const next =
          event.key === 'Home'
            ? 'generate'
            : event.key === 'End'
              ? 'delete'
              : tab === 'generate'
                ? 'delete'
                : 'generate';
        activate(next);
        get(`${next}-tab`).focus();
      }
    });
  }

  // Changing the name invalidates the visible selection immediately.
  get('delete-name').addEventListener('input', () => {
    const hasName = get('delete-name').value.length > 0;
    get('delete-clear').hidden = !hasName;
    preview = null;
    render();
    message(hasName ? 'Scan this map to see its files.' : 'Choose a map, then scan to see its files.');
  });

  get('delete-clear').addEventListener('click', () => {
    const input = get('delete-name');
    input.value = '';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.focus();
  });

  get('delete-search').addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!native || busy) {
      return;
    }
    preview = null;
    render();
    lock(true);
    message('Scanning files…');
    try {
      preview = await invoke('delete_scan', {
        name: get('delete-name').value.trim(),
      });
      render();
      message(
        preview.items.length
          ? 'Select files or whole sections. Review the list before deleting.'
          : 'No matching files found.',
      );
    } catch (error) {
      message(String(error), true);
    } finally {
      lock(false);
    }
  });

  get('delete-all').addEventListener('change', () => {
    for (const { input } of checks) {
      input.checked = get('delete-all').checked;
    }
    updateSelection();
  });

  get('delete-review').addEventListener('click', () => {
    const selected = checks.filter(({ input }) => {
      return input.checked;
    });
    if (busy || !preview || !selected.length) {
      return;
    }
    const dialog = get('delete-confirm');
    get('delete-summary').textContent =
      `${preview.name}: ${selected.length} files selected.`;
    get('delete-confirm-list').replaceChildren();
    // Keep the same three categories as the selection screen, with a visible
    // heading for each selected group rather than one uninterrupted file list.
    for (const [section, [title]] of sections.entries()) {
      const members = selected.filter(({ item }) => {
        return item.section === section;
      });
      if (!members.length) {
        continue;
      }

      const group = document.createElement('section');
      group.className = 'delete-review-group';
      const heading = document.createElement('h3');
      heading.textContent = `${section + 1}. ${title}`;
      const count = document.createElement('span');
      count.textContent = `${members.length} ${members.length === 1 ? 'file' : 'files'}`;
      heading.append(count);
      const list = document.createElement('ul');

      for (const { item } of members) {
        const row = document.createElement('li');
        const root = item.section === 2 ? preview.appdata : context.root;
        const location = item.section === 2 ? 'AppData' : 'WaW';
        row.textContent = `${location}/${item.path}`;
        row.title = `${root}/${item.path}`;
        list.append(row);
      }

      group.append(heading, list);
      get('delete-confirm-list').append(group);
    }
    dialog.returnValue = 'cancel';
    dialog.addEventListener(
      'close',
      async () => {
        if (dialog.returnValue !== 'delete') {
          return;
        }
        lock(true);
        message('Deleting selected files…');
        try {
          const count = await invoke('delete_apply', {
            selected: selected.map(({ item }) => {
              return item.id;
            }),
          });
          message(
            `${count} ${count === 1 ? 'file' : 'files'} deleted from ${preview.name}. Scan again to see what remains.`,
            false,
            true,
          );
          get('status').textContent = 'Deletion complete';
          // Clear directly so the input handler does not replace the success
          // message. Refresh suggestions from the files that remain on disk.
          get('delete-name').value = '';
          get('delete-clear').hidden = true;
          get('delete-maps').replaceChildren();
          try {
            await loadMaps();
          } catch (error) {
            get('delete-message').textContent +=
              ` Could not refresh the map list: ${String(error)}`;
          }
        } catch (error) {
          message(String(error), true);
          get('status').textContent = 'Deletion stopped';
        } finally {
          preview = null;
          render();
          lock(false);
        }
      },
      { once: true },
    );
    dialog.showModal();
    get('delete-cancel').focus();
  });

  get('delete-cancel').onclick = () => {
    get('delete-confirm').close('cancel');
  };
  get('delete-confirm-button').onclick = () => {
    get('delete-confirm').close('delete');
  };

  render();
  lock(false);
  if (!native) {
    message(
      'Browser preview only. Open the desktop app to scan and delete files.',
    );
  }
}
