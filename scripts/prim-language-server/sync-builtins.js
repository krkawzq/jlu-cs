'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

function extractBuiltins(source) {
  const registry = /\bregistry!\s*\{([^}]+)\}/.exec(source)?.[1];
  if (!registry) throw new Error('找不到 prim-rs 的 registry! 内置函数表。');
  const entries = [...registry.matchAll(/\b\w+\s*=>\s*\(\s*"([^"]+)"\s*,\s*(\d+)\s*\)/g)]
    .map(([, name, arity]) => ({ name, arity: Number(arity) }));
  if (!entries.length || entries.length !== new Set(entries.map(e => e.name)).size
      || !entries.some(e => e.name === 'jit') || entries.some(e => e.name !== 'jit' && !e.name.startsWith('std.'))) {
    throw new Error('prim-rs 内置函数表为空、重复或包含未知命名空间。');
  }
  return { source: 'src/prim-rs/src/builtins.rs', registrySha256: createHash('sha256').update(registry).digest('hex'), entries };
}

if (require.main === module) {
  const root = path.resolve(__dirname, '../..');
  const output = path.join(root, 'src/prim-language-server/server/builtin-signatures.json');
  const catalog = extractBuiltins(fs.readFileSync(path.join(root, 'src/prim-rs/src/builtins.rs'), 'utf8'));
  const text = JSON.stringify(catalog, null, 2) + '\n';
  if (process.argv.includes('--check')) {
    if (fs.readFileSync(output, 'utf8') !== text) throw new Error('标准库签名未同步，请运行 node scripts/prim-language-server/sync-builtins.js。');
  } else fs.writeFileSync(output, text);
  const { createInterfaces } = require('../../src/prim-language-server/server/interfaces');
  const { loadBuiltinSources, validateBuiltins } = require('../../src/prim-language-server/server/builtins');
  const sourceDirectory = path.join(root, 'builtin');
  const environment = createInterfaces(loadBuiltinSources(sourceDirectory));
  const errors = validateBuiltins(environment);
  if (errors.length) throw new Error(errors.join('\n'));
  const bundledDirectory = path.join(root, 'src/prim-language-server/builtin');
  const names = fs.readdirSync(sourceDirectory).filter(name => name.endsWith('.pri')).sort();
  if (process.argv.includes('--check')) {
    const bundledNames = fs.readdirSync(bundledDirectory).filter(name => name.endsWith('.pri')).sort();
    if (JSON.stringify(names) !== JSON.stringify(bundledNames) || names.some(name =>
      fs.readFileSync(path.join(sourceDirectory, name), 'utf8') !== fs.readFileSync(path.join(bundledDirectory, name), 'utf8'))) {
      throw new Error('.pri 打包副本未同步，请运行 sync-builtins.js。');
    }
  } else {
    fs.mkdirSync(bundledDirectory, { recursive: true });
    for (const name of fs.readdirSync(bundledDirectory).filter(name => name.endsWith('.pri') && !names.includes(name))) fs.unlinkSync(path.join(bundledDirectory, name));
    for (const name of names) fs.copyFileSync(path.join(sourceDirectory, name), path.join(bundledDirectory, name));
  }
  console.log(`prim-rs 标准库签名：${catalog.entries.length} 项。`);
}

module.exports = { extractBuiltins };
