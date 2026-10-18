// Usage: node check-vitest-json.cjs <report.json> <file-suffix> <min-passed-in-file>
const fs = require("fs");
const [report, suffix, min] = process.argv.slice(2);
const r = JSON.parse(fs.readFileSync(report, "utf8"));
const file = r.testResults.find((t) => t.name.split("\\").join("/").endsWith(suffix));
const passed = file ? file.assertionResults.filter((a) => a.status === "passed").length : 0;
const ok = r.numFailedTests === 0 && r.numPendingTests === 0 && r.numTodoTests === 0 && passed >= Number(min);
if (!ok) console.error(`failed=${r.numFailedTests} pending=${r.numPendingTests} todo=${r.numTodoTests} filePassed=${passed}`);
process.exit(ok ? 0 : 1);
