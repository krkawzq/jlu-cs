'use strict';

const path = require('path');
const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');
let client;

async function activate(context) {
  const interfaces = vscode.workspace.createFileSystemWatcher('**/*.pri');
  context.subscriptions.push(interfaces);
  const serverOptions = { module: context.asAbsolutePath(path.join('server', 'server.js')), transport: TransportKind.ipc };
  client = new LanguageClient('prim', 'Prim Language Server', serverOptions, {
    documentSelector: ['prim', 'prim-interface'].flatMap(language => [{ language, scheme: 'file' }, { language, scheme: 'untitled' }]),
    synchronize: { configurationSection: 'prim', fileEvents: interfaces }
  });
  context.subscriptions.push(client);
  await client.start();
}

function deactivate() { return client?.stop(); }
module.exports = { activate, deactivate };
