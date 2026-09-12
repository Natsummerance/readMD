// tests/test_pet_astra_and_flip.js
const assert = require('assert');
const fs = require('fs');
const path = require('path');

console.log('--- Testing Astra Pets, Builtin Protection & Smart Bubble Flip ---');

const elements = {};
function mockElement(id) {
  return {
    id,
    classList: {
      _classes: new Set(),
      add(c) { this._classes.add(c); },
      remove(c) { this._classes.delete(c); },
      contains(c) { return this._classes.has(c); }
    },
    style: {},
    textContent: '',
    checked: false,
    options: [],
    value: '',
    replaceChildren(...opts) {
      this.options = opts;
      this.value = opts.length > 0 ? opts[0].value : '';
    },
    add(opt) {
      this.options.push(opt);
    },
    _rect: { width: 120, height: 150, left: 200, top: 200 },
    getBoundingClientRect() {
      return this._rect;
    },
    querySelector() { return null; },
    addEventListener() {},
    removeEventListener() {}
  };
}

global.Option = function(text, value) {
  return { text, value };
};

elements['readmd-pet-widget'] = mockElement('readmd-pet-widget');
elements['pet-bubble'] = mockElement('pet-bubble');
elements['pet-bubble-text'] = mockElement('pet-bubble-text');
elements['pet-gallery'] = mockElement('pet-gallery');
elements['pet-gallery-delete'] = mockElement('pet-gallery-delete');
elements['pet-character'] = mockElement('pet-character');
elements['pet-preview-character'] = mockElement('pet-preview-character');
elements['pet-install'] = mockElement('pet-install');
elements['pet-runtime'] = mockElement('pet-runtime');
elements['pet-status-dot'] = mockElement('pet-status-dot');
elements['pet-status-text'] = mockElement('pet-status-text');
elements['pet-status-line'] = mockElement('pet-status-line');

global.$ = id => elements[id] || null;
global.window = {
  innerWidth: 1024,
  innerHeight: 768,
  addEventListener() {},
  removeEventListener() {},
  i18n: {
    t(key) {
      const dict = {
        'pet.gallery.hermes': 'Hermes 伴读使者',
        'pet.preset.mochi': '糯米 / Mochi',
        'pet.preset.moss': '苔苔 / Moss',
        'pet.preset.amber': '琥珀 / Amber',
        'pet.statusRunning': '运行中',
        'pet.statusInAppActive': '桌宠伴读小组件已在阅读器内运行',
      };
      return dict[key] || key;
    }
  }
};
global.document = {
  querySelector() { return null; }
};

// Load pet-batch.js
const code = fs.readFileSync(path.join(__dirname, '../assets/js/features/pet-batch.js'), 'utf-8');
eval(code);

// 1. Test Smart Bubble Flip
console.log('1. Testing bubble smart flip when dragged near viewport top...');
elements['readmd-pet-widget']._rect.top = 200; // Normal middle position
window.showPetBubble('Test bubble 1');
assert.strictEqual(elements['pet-bubble'].classList.contains('is-flipped'), false, 'Should not flip at top=200');

elements['readmd-pet-widget']._rect.top = 50; // Dragged near top (<120px)
window.showPetBubble('Test bubble 2');
assert.strictEqual(elements['pet-bubble'].classList.contains('is-flipped'), true, 'Should flip downwards at top=50');

elements['readmd-pet-widget']._rect.top = 300; // Dragged back down
window.showPetBubble('Test bubble 3');
assert.strictEqual(elements['pet-bubble'].classList.contains('is-flipped'), false, 'Should revert flip at top=300');
console.log('  [PASS] Smart Bubble Flip verified.');

// 2. Test Builtin Pet Gallery & Delete Protection
console.log('2. Testing builtin pet gallery loading & delete button visibility...');
global.apiFetch = async (url) => {
  if (url === '/api/pets') {
    return {
      ok: true,
      json: async () => ({
        ok: true,
        active: 'mochi',
        pets: [
          { slug: 'mochi', display_name: '糯米 / Mochi', is_builtin: true },
          { slug: 'moss', display_name: '苔苔 / Moss', is_builtin: true },
          { slug: 'amber', display_name: '琥珀 / Amber', is_builtin: true },
          { slug: 'custom-cat', display_name: 'My Custom Cat', is_builtin: false }
        ]
      })
    };
  }
  return { ok: false, json: async () => ({ ok: false }) };
};

(async () => {
  await window.refreshPetGallery();
  const select = elements['pet-gallery'];
  assert.strictEqual(select.options.length, 5, 'Should have Hermes + 3 builtins + 1 custom = 5 options');
  assert.strictEqual(select.options[0].text, 'Hermes 伴读使者');
  assert.strictEqual(select.options[1].text, '糯米 / Mochi');
  assert.strictEqual(select.options[2].text, '苔苔 / Moss');
  assert.strictEqual(select.options[3].text, '琥珀 / Amber');
  assert.strictEqual(select.options[4].text, 'My Custom Cat');

  // Verify active background image
  assert.strictEqual(elements['pet-character'].style.backgroundImage, 'url("/api/pets/thumb?slug=mochi")');

  // Verify delete button is hidden for builtin pet mochi
  window.updatePetDeleteButtonVisibility('mochi');
  assert.strictEqual(elements['pet-gallery-delete'].classList.contains('hidden'), true, 'Delete button MUST be hidden for mochi');

  window.updatePetDeleteButtonVisibility('moss');
  assert.strictEqual(elements['pet-gallery-delete'].classList.contains('hidden'), true, 'Delete button MUST be hidden for moss');

  window.updatePetDeleteButtonVisibility('amber');
  assert.strictEqual(elements['pet-gallery-delete'].classList.contains('hidden'), true, 'Delete button MUST be hidden for amber');

  window.updatePetDeleteButtonVisibility('');
  assert.strictEqual(elements['pet-gallery-delete'].classList.contains('hidden'), true, 'Delete button MUST be hidden for Hermes');

  // Verify delete button is SHOWN for custom pet
  window.updatePetDeleteButtonVisibility('custom-cat');
  assert.strictEqual(elements['pet-gallery-delete'].classList.contains('hidden'), false, 'Delete button MUST be visible for custom-cat');

  console.log('  [PASS] Builtin pet gallery & delete protection verified.');

  // 3. Test In-App settings status
  console.log('3. Testing in-app status rendering...');
  elements['pet-runtime'].value = 'in-app';
  window.renderPetSettings({
    enabled: true,
    in_app: true,
    installed: false, // Even if electron adapter is not installed
    preferences: { renderer: 'hermes-sprite', scale: 0.33, opacity: 1.0 }
  });

  assert.strictEqual(elements['pet-install'].classList.contains('hidden'), true, 'Install button should be hidden in in-app mode');
  assert.strictEqual(elements['pet-status-dot'].classList.contains('is-running'), true, 'Status dot should be running');
  assert.strictEqual(elements['pet-status-text'].textContent, '运行中');
  assert.strictEqual(elements['pet-status-line'].textContent, '桌宠伴读小组件已在阅读器内运行');
  console.log('  [PASS] In-App settings status verified.');

  console.log('\nAll Astra & Pet UI tests PASSED successfully!');
  process.exit(0);
})();
