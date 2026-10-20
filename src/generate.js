const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function validate({ name, display = '', source }) {
  if (!name || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
    throw new Error(
      'Use letters, numbers and underscores for the map name. Start with a letter or underscore.',
    );
  }

  if (/^(con|prn|aux|nul|com[0-9]|lpt[0-9])$/i.test(name)) {
    throw new Error(
      'That map name is reserved by Windows. Choose another name.',
    );
  }

  if (!/^[A-Za-z0-9 ]*$/.test(display)) {
    throw new Error('Use letters, numbers and spaces for the display name.');
  }

  if (!['setup', 'blank', 'custom'].includes(source)) {
    throw new Error('Choose a map source.');
  }
}

// UGX inFileTextChange reads lines, replaces text, and writes CRLF after every
// line, even when the placeholder does not occur. Untouched files retain bytes.
function change(bytes, from, to) {
  const text = decoder.decode(bytes);
  if (!text) {
    return new Uint8Array();
  }

  const lines = text.split(/\r\n|\n|\r/);
  if (lines.at(-1) === '') {
    lines.pop();
  }

  return encoder.encode(
    lines
      .map((line) => {
        return line.split(from).join(to) + '\r\n';
      })
      .join(''),
  );
}

// Build the output in memory. The Rust side handles existing files and disk writes.
export function generate(options, templates, customMap) {
  validate(options);
  const { name, display = '', source } = options;
  if (source === 'custom' && !(customMap instanceof Uint8Array)) {
    throw new Error('Choose a custom .map file.');
  }

  const files = new Map();
  const mod = `mods/${name}/`;

  // Copy each resource so substitutions cannot change the shared templates.
  function place(path, resource) {
    if (!templates[resource]) {
      throw new Error(`Missing starter template: ${resource}`);
    }
    files.set(path, templates[resource].slice());
  }

  function edit(path, key, value) {
    files.set(path, change(files.get(path), `UGX_SCRIPTPLACER_${key}`, value));
  }

  // UGX uses the Setup patch map for Custom maps too.
  const mapType = source === 'blank' ? 'blank' : 'setup';
  if (source === 'custom') {
    files.set(`map_source/${name}.map`, customMap.slice());
  } else {
    place(`map_source/${name}.map`, `MAP_nazi_zombie_example_${mapType}.map`);
  }
  place(
    `map_source/${name}_patch.map`,
    `MAP_nazi_zombie_example_${mapType}_patch.map`,
  );
  const mapping = {
    [`bin/Launcher/map_settings/${name}.cfg`]: 'CFG_nazi_zombie_example.cfg',
    [`bin/Launcher/map_settings/${name}_patch.cfg`]:
      'CFG_nazi_zombie_example_patch.cfg',
    [mod + `clientscripts/createfx/${name}_fx.csc`]:
      'CSC_nazi_zombie_example_fx.csc',
    [mod + `clientscripts/${name}.csc`]: 'CSC_nazi_zombie_example.csc',
    [mod + `clientscripts/${name}_amb.csc`]: 'CSC_nazi_zombie_example_amb.csc',
    [mod + `maps/createart/${name}_art.gsc`]: 'GSC_nazi_zombie_example_art.gsc',
    [mod + `maps/createfx/${name}_fx.gsc`]: 'GSC_nazi_zombie_example_fx.gsc',
    [mod + 'maps/mapsTable.csv']: 'CSV_mapsTable.csv',
    [mod + `maps/${name}.gsc`]: 'GSC_nazi_zombie_example.gsc',
    [mod + 'maps/_loadout.gsc']: 'GSC__loadout.gsc',
    [`zone_source/${name}.csv`]: 'CSV_nazi_zombie_example.csv',
    [`zone_source/${name}_patch.csv`]: 'CSV_nazi_zombie_example_patch.csv',
    [mod + 'DLC3_Weapons.iwd']: 'IWD_DLC3_Weapons.iwd',
    [mod + 'mod.arena']: 'ARENA_mod.arena',
    [mod + 'mod.csv']: 'CSV_mod.csv',
    [mod + `${name}.files`]: 'FILES_nazi_zombie_example.files',
  };

  for (const [path, resource] of Object.entries(mapping)) {
    place(path, resource);
  }

  // Keep UGX's substitution order, including clearing unused FX placeholders.
  // Reordering these passes can change the resulting bytes and line endings.
  const main = mod + `maps/${name}.gsc`;
  edit(main, 'WEATHER_INIT', '');
  edit(main, 'WEATHER_THREAD', '');
  edit(`zone_source/${name}.csv`, 'WEATHER_EFX', '');
  edit(mod + 'maps/mapsTable.csv', 'MISSION_SCREEN', `mission_screen_${name}`);
  edit(main, 'FX_INIT', '');
  edit(main, 'FX_THREAD', '');
  for (const path of [
    `bin/Launcher/map_settings/${name}.cfg`,
    `bin/Launcher/map_settings/${name}_patch.cfg`,
    mod + 'mod.arena',
    mod + `clientscripts/${name}.csc`,
    mod + 'maps/mapsTable.csv',
    `zone_source/${name}.csv`,
  ]) {
    edit(path, 'MAPNAME', name);
  }
  edit(`zone_source/${name}.csv`, 'UGXMOD_VISION', 'zombie_factory.vision');
  edit(
    mod + `maps/createart/${name}_art.gsc`,
    'UGXMOD_VISION',
    'zombie_factory',
  );
  edit(mod + `${name}.files`, 'MAPNAME', name);
  edit(main, 'MAPNAME', name);

  // UGX treats an empty or spaces-only display name as the map name.
  const title = display.replaceAll(' ', '') ? display : name;
  for (const path of [mod + 'maps/mapsTable.csv', mod + 'mod.arena', main]) {
    edit(path, 'DISPLAY', title);
  }

  return [...files].map(([path, bytes]) => {
    return { path, bytes };
  });
}
