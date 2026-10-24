// Usage: node check-vitest-json.cjs <report.json> [<file-suffix> <min-passed-in-file>]
// Fails on any failed/pending(skipped)/todo test or an empty run; with a suffix, also requires that file to have >= min passes.
const fs = require("fs");
const [report, suffix, min] = process.argv.slice(2);
const r = JSON.parse(fs.readFileSync(report, "utf8"));
let ok = r.numFailedTests === 0 && r.numPendingTests === 0 && r.numTodoTests === 0 && r.numTotalTests > 0;
let passed = r.numPassedTests;
if (suffix) {
  const file = r.testResults.find((t) => t.name.split("\\").join("/").endsWith(suffix));
  passed = file ? file.assertionResults.filter((a) => a.status === "passed").length : 0;
  ok = ok && passed >= Number(min);
}
if (!ok) console.error(`failed=${r.numFailedTests} pending=${r.numPendingTests} todo=${r.numTodoTests} total=${r.numTotalTests} passed=${passed}`);
else console.log(`vitest: ${passed} passed, none skipped`);
process.exit(ok ? 0 : 1);
