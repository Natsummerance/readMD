// tests/test_pet_queue.js — Node 内置测试，无第三方依赖
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

test('test_reproduce_bug_007', async () => {
  const source = fs.readFileSync(
    'assets/js/features/pet-batch.js', 'utf8');
  const context = vm.createContext({ window: {}, console });
  // 只加载实际生产代码中的队列及其前置声明。
  vm.runInContext(source.split(
    'window.handlePetDroppedFiles =')[0], context);
  vm.runInContext(`
    globalThis.seen = [];
    handlePetDroppedFiles = async (paths) => {
      seen.push(paths[0]);
      if (paths[0] === "broken.md") throw Error("read failed");
    };
  `, context);
  await Promise.allSettled([
    vm.runInContext("receivePetBatch(['broken.md'])", context),
    vm.runInContext("receivePetBatch(['good.md'])", context),
  ]);
  assert.deepEqual(Array.from(context.seen),
    ['broken.md', 'good.md']);
});
