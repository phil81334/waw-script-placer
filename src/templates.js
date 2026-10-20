// Serve as ordinary files: Radiant .map files are not JavaScript source maps.
const names = [
  'ARENA_mod.arena',
  'CFG_nazi_zombie_example.cfg',
  'CFG_nazi_zombie_example_patch.cfg',
  'CSC_nazi_zombie_example.csc',
  'CSC_nazi_zombie_example_amb.csc',
  'CSC_nazi_zombie_example_fx.csc',
  'CSV_mapsTable.csv',
  'CSV_mod.csv',
  'CSV_nazi_zombie_example.csv',
  'CSV_nazi_zombie_example_patch.csv',
  'FILES_nazi_zombie_example.files',
  'GSC__loadout.gsc',
  'GSC_nazi_zombie_example.gsc',
  'GSC_nazi_zombie_example_art.gsc',
  'GSC_nazi_zombie_example_fx.gsc',
  'IWD_DLC3_Weapons.iwd',
  'MAP_nazi_zombie_example_blank.map',
  'MAP_nazi_zombie_example_blank_patch.map',
  'MAP_nazi_zombie_example_setup.map',
  'MAP_nazi_zombie_example_setup_patch.map',
];

// Read bytes rather than text to preserve each original file's encoding and endings.
export async function loadTemplates() {
  return Object.fromEntries(
    await Promise.all(
      names.map(async (name) => {
        const response = await fetch(`/templates/${name}`);
        if (!response.ok) {
          throw new Error(`Cannot load ${name}`);
        }

        return [name, new Uint8Array(await response.arrayBuffer())];
      }),
    ),
  );
}
