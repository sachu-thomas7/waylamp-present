// Only one Tauri asset root exists (controller/dist) -- relocate presentation's build under it.
const fs = require('node:fs');
const path = require('node:path');

const src = path.resolve(__dirname, '..', 'presentation', 'dist');
const dest = path.resolve(__dirname, '..', 'controller', 'dist', 'presentation');

fs.rmSync(dest, { recursive: true, force: true });
fs.cpSync(src, dest, { recursive: true });
console.log(`merge-presentation-dist: ${src} -> ${dest}`);
