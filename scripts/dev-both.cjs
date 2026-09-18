// Presentation window loads :5174 directly (src-tauri/src/app.rs) -- keep both dev servers alive.
const { spawn } = require('node:child_process');

function run(name, cwd) {
  const proc = spawn('npm', ['run', 'dev'], { cwd, stdio: 'inherit', shell: true });
  proc.on('exit', (code) => {
    console.error(`[${name}] dev server exited (code ${code})`);
    process.exit(code ?? 1);
  });
  return proc;
}

const children = [run('controller', 'controller'), run('presentation', 'presentation')];

for (const sig of ['SIGINT', 'SIGTERM']) {
  process.on(sig, () => {
    for (const c of children) c.kill();
    process.exit(0);
  });
}
