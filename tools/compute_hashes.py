import hashlib

with open('third_party/hermes-agent-pet/apps/desktop/src/app/pet-overlay/pet-overlay-app.tsx', 'r', encoding='utf-8') as f:
    u = f.read()

o = '      setComposerOpen(open => !open)'
r = "      window.hermesDesktop?.petOverlay?.control({ type: 'open-menu' })"
h = '// GENERATED FROM THE PINNED HERMES SOURCE SNAPSHOT. DO NOT EDIT.\n// ReadMD host adaptation: single click emits open-menu; double click remains toggle-app.\n\n'
g = h + u.replace(o, r)
sha = hashlib.sha256(g.encode('utf-8')).hexdigest()
print('Exact generated_source_sha256:', sha)
