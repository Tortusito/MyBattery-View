// The window scripts aren't loaded by any other test, so a typo there would only show up as a
// dead screen in the app. Compile each one to catch that here.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const dir = path.join(__dirname, '..', 'src', 'renderer');
for (const file of fs.readdirSync(dir).filter((f) => f.endsWith('.js'))) {
  test(`${file} compiles`, () => {
    assert.doesNotThrow(() => new vm.Script(fs.readFileSync(path.join(dir, file), 'utf8'), { filename: file }));
  });
}
