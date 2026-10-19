import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import 'monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController';
import 'monaco-editor/esm/vs/editor/contrib/hover/browser/contentHoverController';
import 'monaco-editor/esm/vs/editor/contrib/gotoSymbol/browser/goToCommands';
import 'monaco-editor/esm/vs/editor/contrib/rename/browser/rename';
import 'monaco-editor/esm/vs/editor/contrib/parameterHints/browser/parameterHints';
import 'monaco-editor/esm/vs/editor/contrib/inlayHints/browser/inlayHintsContribution';
import 'monaco-editor/esm/vs/editor/contrib/semanticTokens/browser/documentSemanticTokens';
import 'monaco-editor/esm/vs/editor/contrib/folding/browser/folding';
import 'monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution';
import 'monaco-editor/esm/vs/basic-languages/java/java.contribution';
import 'monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution';
import 'monaco-editor/esm/vs/basic-languages/rust/rust.contribution';
import 'monaco-editor/esm/vs/basic-languages/python/python.contribution';
import 'monaco-editor/esm/vs/basic-languages/scheme/scheme.contribution';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import * as textmate from 'vscode-textmate';
import * as oniguruma from 'vscode-oniguruma';
import wasmUrl from 'vscode-oniguruma/release/onig.wasm?url';
import {Terminal} from '@xterm/xterm';
import {FitAddon} from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import {PrimLanguageClient} from './lsp';

window.MonacoEnvironment={getWorker:()=>new EditorWorker()};
const token=document.querySelector('meta[name="prim-token"]').content;
let languageClient, editor, terminal,fit,terminalSocket,activeModel;
const readModels=new Map(),modelListeners=new Map();
const listeners=new Set();
const status=text=>{for(const callback of listeners)callback(text);};
const announce=model=>window.dispatchEvent(new CustomEvent('prim-model',{detail:{language:model.getLanguageId()}}));
const scope=token=>{
  const scopes=token.scopes.join(' ');
  if(/comment/.test(scopes))return 'comment';
  if(/constant.character.escape/.test(scopes))return 'string.escape';
  if(/string/.test(scopes))return 'string';
  if(/constant.numeric/.test(scopes))return 'number';
  if(/keyword|constant.language/.test(scopes))return 'keyword';
  if(/entity.name.function|support.function/.test(scopes))return 'function';
  if(/variable.parameter/.test(scopes))return 'parameter';
  if(/property/.test(scopes))return 'property';
  if(/entity.name.type/.test(scopes))return 'type';
  return 'identifier';
};
class TokenState{constructor(stack=textmate.INITIAL){this.stack=stack;}clone(){return new TokenState(this.stack);}equals(other){return other instanceof TokenState&&this.stack.equals(other.stack);}}
async function setupGrammar(){
  const extension=await fetch('/api/extension').then(r=>r.json());
  await oniguruma.loadWASM(await fetch(wasmUrl).then(r=>r.arrayBuffer()));
  const registry=new textmate.Registry({onigLib:Promise.resolve({createOnigScanner:s=>new oniguruma.OnigScanner(s),createOnigString:s=>new oniguruma.OnigString(s)}),loadGrammar:async name=>{const contribution=extension.manifest.contributes.grammars.find(g=>g.scopeName===name);return contribution?extension.grammars[contribution.path.split('/').at(-1)]:null;}});
  for(const language of extension.manifest.contributes.languages){
    monaco.languages.register({id:language.id,extensions:language.extensions,aliases:language.aliases});
    const config=structuredClone(extension.configuration);config.wordPattern=new RegExp(config.wordPattern);for(const key of Object.keys(config.indentationRules))config.indentationRules[key]=new RegExp(config.indentationRules[key]);
    monaco.languages.setLanguageConfiguration(language.id,config);
    const grammar=await registry.loadGrammar(extension.manifest.contributes.grammars.find(g=>g.language===language.id).scopeName);
    monaco.languages.setTokensProvider(language.id,{getInitialState:()=>new TokenState(),tokenize:(line,state)=>{const r=grammar.tokenizeLine(line,state.stack);return {endState:new TokenState(r.ruleStack),tokens:r.tokens.map(t=>({startIndex:t.startIndex,scopes:scope(t)}))};}});
  }
  monaco.editor.defineTheme('prim-warm',{base:'vs-dark',inherit:true,rules:[{token:'keyword',foreground:'E8957A'},{token:'function',foreground:'E9C48E'},{token:'parameter',foreground:'E2B4C4'},{token:'property',foreground:'A4C3DF'},{token:'string',foreground:'BCCF98'},{token:'number',foreground:'E5A86E'},{token:'comment',foreground:'8A877D',fontStyle:'italic'},{token:'type',foreground:'A9CFC4'},{token:'variable',foreground:'ECE9E0'},{token:'namespace',foreground:'A9CFC4'},{token:'identifier',foreground:'ECE9E0'}],colors:{'editor.background':'#1F1E1D','editorBracketHighlight.foreground1':'#D4A27F','editorBracketHighlight.foreground2':'#E2B4C4','editorBracketHighlight.foreground3':'#A4C3DF','editorBracketMatch.background':'#3A383400','editorBracketMatch.border':'#6B6A64','editor.foreground':'#ECE9E0','editorLineNumber.foreground':'#5F5D57','editorLineNumber.activeForeground':'#B3AFA3','editor.lineHighlightBackground':'#292826','editor.lineHighlightBorder':'#00000000','editor.selectionBackground':'#5A4134','editorCursor.foreground':'#D97757','editorInlayHint.foreground':'#A8A49A','editorInlayHint.background':'#2C2B28','editorWidget.background':'#2A2926','editorSuggestWidget.background':'#2A2926','editorSuggestWidget.selectedBackground':'#3F3A35','editorHoverWidget.background':'#2A2926','editorIndentGuide.background1':'#2F2E2B','scrollbarSlider.background':'#4A484380'}});
  monaco.editor.setTheme('prim-warm');
  return extension;
}
const ready=setupGrammar();
async function initEditor(element,workspace,onChange){
  await ready;
  if(editor)return editor;
  editor=monaco.editor.create(element,{theme:'prim-warm',fontFamily:'"JetBrains Mono", "Noto Sans Mono CJK SC", monospace',fontSize:15,lineHeight:24,fontLigatures:false,occurrencesHighlight:'off',automaticLayout:true,minimap:{enabled:false},scrollBeyondLastLine:false,tabSize:4,wordWrap:'off',padding:{top:14,bottom:14},semanticHighlighting:{enabled:true},'semanticHighlighting.enabled':true,inlayHints:{enabled:'on'},bracketPairColorization:{enabled:true},ariaLabel:'Prim 实战代码编辑器'});
  editor.addCommand(monaco.KeyMod.CtrlCmd|monaco.KeyCode.Enter,()=>window.dispatchEvent(new CustomEvent('prim-run')));
  languageClient=new PrimLanguageClient(monaco,workspace,token,status);
  try{await languageClient.connect();}catch(error){status(`语言服务不可用：${error.message}`);}
  monaco.editor.registerEditorOpener({openCodeEditor:async(source,resource,selection)=>{try{const model=await languageClient.loadDocument(resource.toString());editor.setModel(model);editor.updateOptions({readOnly:model.getLanguageId()==='prim-interface'});announce(model);if(selection){editor.setSelection(selection);editor.revealRangeInCenter(selection);}return true;}catch{return false;}}});
  editor.onDidChangeModelContent(()=>{if(activeModel===editor.getModel())onChange?.(activeModel.getValue());});
  return editor;
}
function setCode(id,code,workspace,language='prim'){
  const uri=monaco.Uri.parse(`${workspace}/${id}.${language==='python'?'py':'pr'}`);
  let model=monaco.editor.getModel(uri);
  if(!model){model=monaco.editor.createModel(code,language,uri);languageClient?.attach(model);}else if(model.getValue()!==code)model.setValue(code);
  activeModel=model;editor.setModel(model);editor.updateOptions({readOnly:false});editor.layout();announce(model);return model;
}
async function colorize(code,language){await ready;return monaco.editor.colorize(code,language,{tabSize:2});}
async function initTerminal(element,onExit){
  if(terminal)return terminal;
  await document.fonts.load('16px "JetBrains Mono"');
  terminal=new Terminal({fontFamily:'"JetBrains Mono", "Noto Sans Mono CJK SC", monospace',fontSize:14,lineHeight:1.2,convertEol:false,scrollback:3000,cursorBlink:true,theme:{background:'#1F1E1D',foreground:'#ECE9E0',cursor:'#D97757',cursorAccent:'#1F1E1D',selectionBackground:'#5A4134',black:'#1F1E1D',red:'#E07A65',green:'#A9C08A',yellow:'#E5C07B',blue:'#8FB3D9',magenta:'#D49BB5',cyan:'#8EC5B8',white:'#D8D4C8',brightBlack:'#6B6A64',brightRed:'#F0937F',brightGreen:'#BCD39B',brightYellow:'#F0D08F',brightBlue:'#A6C4E4',brightMagenta:'#E2B0C7',brightCyan:'#A3D3C7',brightWhite:'#F5F3EC'}});
  fit=new FitAddon();terminal.loadAddon(fit);terminal.open(element);fit.fit();
  terminal.onData(data=>{if(terminalSocket?.readyState===WebSocket.OPEN)terminalSocket.send(JSON.stringify({kind:'input',data}));});
  terminal.onResize(({cols,rows})=>{if(terminalSocket?.readyState===WebSocket.OPEN)terminalSocket.send(JSON.stringify({kind:'resize',cols,rows}));});
  new ResizeObserver(()=>{try{fit.fit();}catch{}}).observe(element);
  return terminal;
}
async function startTerminal(options,onEvent){
  if(terminalSocket?.readyState===WebSocket.OPEN)throw new Error('请先停止当前终端程序');
  terminal.reset();fit.fit();
  const address=new URL('/ws/terminal',location.href);address.protocol=location.protocol==='https:'?'wss:':'ws:';address.searchParams.set('token',token);
  const socket=new WebSocket(address);terminalSocket=socket;socket.binaryType='arraybuffer';
  socket.onopen=()=>{socket.send(JSON.stringify({...options,cols:terminal.cols,rows:terminal.rows}));terminal.focus();};
  socket.onmessage=event=>{if(typeof event.data==='string'){const value=JSON.parse(event.data);onEvent(value);}else{const chunk=new Uint8Array(event.data);terminal.write(chunk,()=>{if(socket.readyState===WebSocket.OPEN)socket.send(JSON.stringify({kind:'ack',bytes:chunk.byteLength}));});}};
  socket.onerror=()=>onEvent({kind:'error',message:'终端连接失败'});
  socket.onclose=()=>onEvent({kind:'closed'});
}
window.PrimEditor={ready,colorize,initEditor,setCode,initTerminal,startTerminal,
  writeResult:result=>{terminal.reset();terminal.write((result.stdout||'（无标准输出）\n').replace(/\r?\n/g,'\r\n'));terminal.write('\r\n\x1b[38;5;173m── 诊断与统计 ──\x1b[0m\r\n');terminal.write((result.stderr||'（无诊断输出）\n').replace(/\r?\n/g,'\r\n'));},
  getLanguage:()=>editor?.getModel()?.getLanguageId(),
  clear:()=>terminal?.reset(),
  setFontSize:(px,terminalPx=px-1)=>{editor?.updateOptions({fontSize:px,lineHeight:Math.round(px*1.6)});terminalPx=Math.max(10,terminalPx);if(terminal&&terminal.options.fontSize!==terminalPx){terminal.options.fontSize=terminalPx;try{fit.fit();}catch{}}},
  getValue:()=>editor?.getValue()||'',setValue:text=>editor?.setValue(text),layout:()=>{editor?.layout();try{fit?.fit();}catch{}},
  stop:()=>{if(terminalSocket?.readyState===WebSocket.OPEN)terminalSocket.send(JSON.stringify({kind:'stop'}));},
  onStatus:callback=>listeners.add(callback),
  debug:()=>({client:languageClient,monaco,editor,terminal,socket:terminalSocket,activeModel}),
  getTerminalText:()=>{if(!terminal)return '';const b=terminal.buffer.active;let s='';for(let i=0;i<b.length;i++)s+=b.getLine(i)?.translateToString(true)+'\n';return s;}
};
window.dispatchEvent(new Event('prim-editor-ready'));
