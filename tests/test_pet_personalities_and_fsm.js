// tests/test_pet_personalities_and_fsm.js
const assert = require('assert');
const fs = require('fs');
const path = require('path');

console.log('--- Testing Pet Personalities, Bubble Hover/Dismiss & FSM ---');

const elements = {};
function mockElement(id) {
  const listeners = {};
  return {
    id,
    classList: {
      _classes: new Set(),
      add(c) { this._classes.add(c); },
      remove(c) { this._classes.delete(c); },
      contains(c) { return this._classes.has(c); },
      toggle(c, force) {
        if (force === undefined) {
          if (this._classes.has(c)) this._classes.delete(c);
          else this._classes.add(c);
        } else if (force) this._classes.add(c);
        else this._classes.delete(c);
      }
    },
    style: {},
    dataset: {},
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
    addEventListener(event, fn) {
      listeners[event] = listeners[event] || [];
      listeners[event].push(fn);
    },
    removeEventListener(event, fn) {
      if (listeners[event]) {
        listeners[event] = listeners[event].filter(cb => cb !== fn);
      }
    },
    trigger(event, data = {}) {
      if (listeners[event]) {
        listeners[event].forEach(cb => cb({ stopPropagation() {}, ...data }));
      }
    }
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
elements['pet-install-runtime'] = mockElement('pet-install-runtime');
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

// 1. Test Bubble Click-to-Dismiss and Hover-to-Pause
console.log('1. Testing bubble Click-to-Dismiss and Hover-to-Pause...');
window.initBubbleInteractions();

// Show a bubble
window.showPetBubble('测试气泡', 4000);
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), true);
assert.strictEqual(elements['pet-bubble-text'].textContent, '测试气泡');

// Click on bubble -> dismisses immediately
elements['pet-bubble'].trigger('click');
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), false, 'Bubble should be dismissed on click');

// Show bubble again and test hover
window.showPetBubble('第二条气泡', 4000);
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), true);
elements['pet-bubble'].trigger('mouseenter');
// Hover pauses timer
elements['pet-bubble'].trigger('mouseleave');
// Mouse leave keeps it visible until scheduled delay
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), true);
window.hidePetBubble();
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), false);
console.log('  [PASS] Bubble click-to-dismiss and hover-to-pause verified.');

// 2. Test Role Personality Dialogue Matrix
console.log('2. Testing role personality dialogue matrix...');

// Select Mochi
elements['pet-gallery'].value = 'mochi';
assert.strictEqual(window.getActivePetSlug(), 'mochi');
const mochiPoke = window.getRoleSpecificQuote('pokesCombo', null, '');
assert.ok(mochiPoke.includes('喵') || mochiPoke.includes('猫') || mochiPoke.includes('小鱼干') || mochiPoke.includes('投降'), `Mochi combo quote should match: ${mochiPoke}`);

const mochiSleep = window.getRoleSpecificQuote('sleeping', null, '');
assert.ok(mochiSleep.includes('小猫球') || mochiSleep.includes('呼噜呼噜'), `Mochi sleep quote should match: ${mochiSleep}`);

// Select Moss
elements['pet-gallery'].value = 'moss';
assert.strictEqual(window.getActivePetSlug(), 'moss');
const mossPoke = window.getRoleSpecificQuote('pokesCombo', null, '');
assert.ok(mossPoke.includes('啵') || mossPoke.includes('史莱姆') || mossPoke.includes('戳'), `Moss combo quote should match: ${mossPoke}`);

const mossSleep = window.getRoleSpecificQuote('sleeping', null, '');
assert.ok(mossSleep.includes('果冻') || mossSleep.includes('啵'), `Moss sleep quote should match: ${mossSleep}`);

// Select Amber
elements['pet-gallery'].value = 'amber';
assert.strictEqual(window.getActivePetSlug(), 'amber');
const amberPoke = window.getRoleSpecificQuote('pokesCombo', null, '');
assert.ok(amberPoke.includes('尾巴') || amberPoke.includes('狐狸') || amberPoke.includes('零食'), `Amber combo quote should match: ${amberPoke}`);

const amberSleep = window.getRoleSpecificQuote('sleeping', null, '');
assert.ok(amberSleep.includes('大尾巴') || amberSleep.includes('小狐狸'), `Amber sleep quote should match: ${amberSleep}`);

// Fallback to default/Hermes
elements['pet-gallery'].value = '';
assert.strictEqual(window.getActivePetSlug(), '');
const defaultPoke = window.getRoleSpecificQuote('pokesCombo', null, '默认戳戳');
assert.strictEqual(defaultPoke, '默认戳戳');

console.log('  [PASS] Role personality dialogue matrix verified.');

// 3. Test In-App vs Desktop Settings Mode UI Adaptation
console.log('3. Testing settings UI adaptation (In-App vs Desktop)...');

// In-App mode: both pet-install and pet-install-runtime must be hidden!
elements['pet-runtime'].value = 'in-app';
window.renderPetSettings({
  enabled: true,
  in_app: true,
  installed: false,
  preferences: { renderer: 'hermes-sprite', scale: 0.33, opacity: 1.0 }
});
assert.strictEqual(elements['pet-install'].classList.contains('hidden'), true, 'pet-install should be hidden in in-app mode');
assert.strictEqual(elements['pet-install-runtime'].classList.contains('hidden'), true, 'pet-install-runtime should be hidden in in-app mode');
assert.strictEqual(elements['pet-status-dot'].classList.contains('is-running'), true);

// Desktop mode: pet-install-runtime must be visible!
elements['pet-runtime'].value = 'desktop';
window.renderPetSettings({
  enabled: false,
  in_app: false,
  installed: false,
  preferences: { renderer: 'hermes-sprite', scale: 0.33, opacity: 1.0 }
});
assert.strictEqual(elements['pet-install-runtime'].classList.contains('hidden'), false, 'pet-install-runtime should be visible in desktop mode');
assert.strictEqual(elements['pet-install'].classList.contains('hidden'), false, 'pet-install should be visible in desktop mode');

console.log('  [PASS] Settings UI adaptation verified.');

console.log('\nAll Pet Personalities, FSM & Interaction tests PASSED successfully!');
process.exit(0);
