import { packager } from '@electron/packager';

const outputs = await packager({
  dir: '.', name: 'Planimulation', executableName: 'planimulation', out: 'release', overwrite: true,
  asar: true, prune: true,
  extraResource: ['dist/native'],
  ignore: [/^\/(src|native|tests|scripts|docs|artifacts|test-results|release)(\/|$)/, /^\/dist\/native(\/|$)/, /^\/\.(git|agents|codex)(\/|$)/],
});
for (const output of outputs) console.log(output);
