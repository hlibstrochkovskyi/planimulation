import { packager } from '@electron/packager';

const outputs = await packager({
  dir: '.', name: 'Planimulation', executableName: 'planimulation', out: 'release', overwrite: true,
  asar: true, prune: true,
  ignore: [/^\/(src|tests|scripts|docs|artifacts|test-results|release)(\/|$)/, /^\/\.(git|agents|codex)(\/|$)/],
});
for (const output of outputs) console.log(output);
