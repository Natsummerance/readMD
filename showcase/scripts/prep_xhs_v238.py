import json
from pathlib import Path
from PIL import Image

src_dir = Path('showcase/output/v238-update')
out_dir = Path('showcase/output/xhs-v238/artifacts')
img_dir = out_dir / 'images'
img_dir.mkdir(parents=True, exist_ok=True)

pngs = sorted([p for p in src_dir.glob('*.png')])
jpg_paths = []

for i, p in enumerate(pngs, 1):
    parts = p.stem.split('-', 1)
    suffix = parts[1] if len(parts) > 1 else parts[0]
    jpg_name = f'xhs-{i:02d}-{suffix}.jpg'
    jpg_path = img_dir / jpg_name
    im = Image.open(p).convert('RGB')
    im.save(jpg_path, 'JPEG', quality=92)
    jpg_paths.append(str(jpg_path.resolve()).replace('\\', '/'))
    print(f'Converted {p.name} -> {jpg_name}')

title = Path('showcase/update-v238/title.txt').read_text(encoding='utf-8').strip()
body = Path('showcase/update-v238/body.txt').read_text(encoding='utf-8').strip()
topics = [line.strip() for line in Path('showcase/update-v238/topics.txt').read_text(encoding='utf-8').splitlines() if line.strip()]

(out_dir / 'title.txt').write_text(title + '\n', encoding='utf-8')
(out_dir / 'body.txt').write_text(body + '\n', encoding='utf-8')
(out_dir / 'topics.txt').write_text('\n'.join(topics) + '\n', encoding='utf-8')

meta = {
    'title': title,
    'body': body,
    'topics': topics,
    'images': jpg_paths,
    'source_urls': [
        'https://github.com/Natsummerance/readMD/releases/tag/v2.3.8',
        'https://github.com/Natsummerance/readMD/compare/v2.3.7...v2.3.8'
    ],
    'version_state': 'release'
}

(out_dir / 'metadata.json').write_text(json.dumps(meta, ensure_ascii=False, indent=2), encoding='utf-8')
print('Artifact package prepared with', len(jpg_paths), 'images.')
