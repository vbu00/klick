// Флаги стран в SVG-файлы для iOS-беты (scripts/ios/build-native.sh): те же рисунки, что у окна.
//   node ui/scripts/export-flags.mjs <папка>   (Node 22.18+ читает .ts сам)

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { FLAGS, flagSvg } from '../src/lib/flags.ts';

const out = process.argv[2];
if (!out) throw new Error('нужна папка');
mkdirSync(out, { recursive: true });
for (const code of Object.keys(FLAGS)) writeFileSync(join(out, `${code}.svg`), flagSvg(code));
console.log(`флагов: ${Object.keys(FLAGS).length}`);
