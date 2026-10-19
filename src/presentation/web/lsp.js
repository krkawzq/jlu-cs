export class PrimLanguageClient {
  constructor(monaco, workspace, token, onStatus) {
    this.monaco=monaco;this.workspace=workspace;this.token=token;this.onStatus=onStatus;
    this.sequence=0;this.pending=new Map();this.documents=new Map();this.disposables=[];this.messages=[];
  }
  async connect() {
    const address=new URL('/ws/lsp',location.href);address.protocol=location.protocol==='https:'?'wss:':'ws:';address.searchParams.set('token',this.token);
    this.socket=new WebSocket(address);
    await new Promise((resolve,reject)=>{this.socket.onopen=resolve;this.socket.onerror=()=>reject(new Error('语言服务连接失败'));});
    this.socket.onmessage=event=>this.receive(JSON.parse(event.data));
    this.socket.onclose=()=>{for(const p of this.pending.values()){clearTimeout(p.timer);p.reject(new Error('语言服务连接已关闭'));}this.pending.clear();this.onStatus('语言服务已断开');};
    const response=await this.request('initialize',{
      processId:null,clientInfo:{name:'Prim Presentation',version:'0.2'},rootUri:this.workspace,
      workspaceFolders:[{uri:this.workspace,name:'课堂实验'}],
      capabilities:{workspace:{configuration:true,workspaceFolders:true},textDocument:{
        synchronization:{didSave:false},completion:{completionItem:{snippetSupport:true,documentationFormat:['markdown','plaintext']}},
        hover:{contentFormat:['markdown','plaintext']},signatureHelp:{signatureInformation:{documentationFormat:['markdown','plaintext']}},
        publishDiagnostics:{versionSupport:true},definition:{linkSupport:true},rename:{prepareSupport:true},
        semanticTokens:{requests:{full:true},tokenTypes:['namespace','type','function','parameter','variable','property'],tokenModifiers:['declaration','readonly','defaultLibrary'],formats:['relative']},
        inlayHint:{},documentSymbol:{hierarchicalDocumentSymbolSupport:true}}}
    });
    this.capabilities=response.capabilities;this.notify('initialized',{});this.notify('workspace/didChangeConfiguration',{settings:{prim:{diagnostics:{warnings:true},inlayHints:{types:true,parameters:false}}}});
    this.register();this.onStatus(`${response.serverInfo?.name||'Prim LSP'} ${response.serverInfo?.version||''} · 已连接`);
    return this;
  }
  request(method,params) {
    const id=++this.sequence;
    return new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{this.pending.delete(id);this.notify('$/cancelRequest',{id});reject(new Error(`${method} 超时`));},10000);
      this.pending.set(id,{resolve,reject,timer});this.socket.send(JSON.stringify({jsonrpc:'2.0',id,method,params}));
    });
  }
  notify(method,params){if(this.socket?.readyState===WebSocket.OPEN)this.socket.send(JSON.stringify({jsonrpc:'2.0',method,params}));}
  receive(message){
    if(message.method){
      if(message.id!==undefined){let result=null;if(message.method==='workspace/configuration')result=message.params.items.map(()=>({}));this.socket.send(JSON.stringify({jsonrpc:'2.0',id:message.id,result}));return;}
      if(message.method==='textDocument/publishDiagnostics'){
        const model=this.monaco.editor.getModel(this.monaco.Uri.parse(message.params.uri));if(!model)return;
        if(message.params.version!==undefined&&message.params.version!==model.getVersionId())return;
        this.monaco.editor.setModelMarkers(model,'prim-lsp',(message.params.diagnostics||[]).map(d=>({...this.range(d.range),message:d.message,severity:[0,8,4,2,1][d.severity||1],code:String(d.code||''),source:'Prim LSP'})));
      }else if(message.method==='window/showMessage'){this.onStatus(message.params.message);}
      this.messages.push(message);if(this.messages.length>30)this.messages.shift();return;
    }
    const pending=this.pending.get(message.id);if(!pending)return;clearTimeout(pending.timer);this.pending.delete(message.id);
    if(message.error)pending.reject(new Error(message.error.message));else pending.resolve(message.result);
  }
  range(r){return {startLineNumber:r.start.line+1,startColumn:r.start.character+1,endLineNumber:r.end.line+1,endColumn:r.end.character+1};}
  position(p){return {line:p.lineNumber-1,character:p.column-1};}
  params(model,position){return {textDocument:{uri:model.uri.toString()},...(position?{position:this.position(position)}:{})};}
  markdown(value){if(!value)return undefined;if(typeof value==='string')return {value,isTrusted:false};return {value:value.value||'',isTrusted:false};}
  attach(model){
    if(this.documents.has(model.uri.toString())||!['prim','prim-interface'].includes(model.getLanguageId()))return;
    this.documents.set(model.uri.toString(),model);
    this.notify('textDocument/didOpen',{textDocument:{uri:model.uri.toString(),languageId:model.getLanguageId(),version:model.getVersionId(),text:model.getValue()}});
    const change=model.onDidChangeContent(()=>this.notify('textDocument/didChange',{textDocument:{uri:model.uri.toString(),version:model.getVersionId()},contentChanges:[{text:model.getValue()}]}));
    model.onWillDispose(()=>{change.dispose();this.documents.delete(model.uri.toString());this.notify('textDocument/didClose',{textDocument:{uri:model.uri.toString()}});});
  }
  async loadDocument(uri){
    const u=this.monaco.Uri.parse(uri);let model=this.monaco.editor.getModel(u);if(model)return model;
    const response=await fetch(`/api/document?uri=${encodeURIComponent(uri)}`,{headers:{'X-Prim-Token':this.token}});
    if(!response.ok)throw new Error('无法读取接口定义');const doc=await response.json();model=this.monaco.editor.createModel(doc.text,doc.language,u);this.attach(model);return model;
  }
  edits(edit){
    const edits=[];
    for(const change of edit?.documentChanges||[]){for(const value of change.edits||[])edits.push({resource:this.monaco.Uri.parse(change.textDocument.uri),versionId:change.textDocument.version??undefined,textEdit:{range:this.range(value.range),text:value.newText}});}
    for(const [uri,changes] of Object.entries(edit?.changes||{})){for(const value of changes)edits.push({resource:this.monaco.Uri.parse(uri),textEdit:{range:this.range(value.range),text:value.newText}});}
    return {edits};
  }
  register(){
    const m=this.monaco,langs=['prim','prim-interface'];const add=x=>this.disposables.push(x);
    const kinds=['Text','Method','Function','Constructor','Field','Variable','Class','Interface','Module','Property','Unit','Value','Enum','Keyword','Snippet','Color','File','Reference','Folder','EnumMember','Constant','Struct','Event','Operator','TypeParameter'];
    for(const language of langs){
      add(m.languages.registerHoverProvider(language,{provideHover:async(model,p)=>{const r=await this.request('textDocument/hover',this.params(model,p));if(!r)return;return {range:r.range?this.range(r.range):undefined,contents:(Array.isArray(r.contents)?r.contents:[r.contents]).map(v=>this.markdown(v))};}}));
      add(m.languages.registerCompletionItemProvider(language,{triggerCharacters:['.','@'],provideCompletionItems:async(model,p)=>{const r=await this.request('textDocument/completion',this.params(model,p));const word=model.getWordUntilPosition(p);return {incomplete:r?.isIncomplete||false,suggestions:(Array.isArray(r)?r:r?.items||[]).map(v=>({label:v.label,kind:m.languages.CompletionItemKind[kinds[(v.kind||1)-1]]??m.languages.CompletionItemKind.Text,detail:v.detail,documentation:this.markdown(v.documentation),insertText:v.textEdit?.newText||v.insertText||v.label,insertTextRules:v.insertTextFormat===2?m.languages.CompletionItemInsertTextRule.InsertAsSnippet:undefined,range:v.textEdit?.range?this.range(v.textEdit.range):new m.Range(p.lineNumber,word.startColumn,p.lineNumber,word.endColumn),sortText:v.sortText,filterText:v.filterText}))};}}));
      const definitions=async(method,model,p)=>{let r=await this.request(method,this.params(model,p));if(!r)return [];r=Array.isArray(r)?r:[r];return Promise.all(r.map(async v=>{const uri=v.uri||v.targetUri;await this.loadDocument(uri);return {uri:m.Uri.parse(uri),range:this.range(v.range||v.targetSelectionRange||v.targetRange)};}));};
      add(m.languages.registerDefinitionProvider(language,{provideDefinition:(model,p)=>definitions('textDocument/definition',model,p)}));
      add(m.languages.registerTypeDefinitionProvider(language,{provideTypeDefinition:(model,p)=>definitions('textDocument/typeDefinition',model,p)}));
      add(m.languages.registerReferenceProvider(language,{provideReferences:async(model,p,context)=>(await this.request('textDocument/references',{...this.params(model,p),context})).map(v=>({uri:m.Uri.parse(v.uri),range:this.range(v.range)}))}));
      add(m.languages.registerRenameProvider(language,{resolveRenameLocation:async(model,p)=>{const v=await this.request('textDocument/prepareRename',this.params(model,p));return v?{range:this.range(v.range),text:v.placeholder}:{range:new m.Range(p.lineNumber,p.column,p.lineNumber,p.column),text:'',rejectReason:'此位置不能重命名'};},provideRenameEdits:async(model,p,newName)=>this.edits(await this.request('textDocument/rename',{...this.params(model,p),newName}))}));
      add(m.languages.registerSignatureHelpProvider(language,{signatureHelpTriggerCharacters:['(',','],signatureHelpRetriggerCharacters:[','],provideSignatureHelp:async(model,p)=>{const value=await this.request('textDocument/signatureHelp',this.params(model,p));if(!value)return;return {value:{...value,signatures:value.signatures.map(s=>({...s,documentation:this.markdown(s.documentation),parameters:(s.parameters||[]).map(v=>({...v,documentation:this.markdown(v.documentation)}))}))},dispose(){}};}}));
      const legend=this.capabilities.semanticTokensProvider?.legend;
      if(legend)add(m.languages.registerDocumentSemanticTokensProvider(language,{getLegend:()=>legend,provideDocumentSemanticTokens:async model=>{const r=await this.request('textDocument/semanticTokens/full',this.params(model));return {data:new Uint32Array(r?.data||[]),resultId:r?.resultId};},releaseDocumentSemanticTokens(){}}));
      add(m.languages.registerInlayHintsProvider(language,{provideInlayHints:async(model,range)=>{const hints=await this.request('textDocument/inlayHint',{...this.params(model),range:{start:{line:range.startLineNumber-1,character:range.startColumn-1},end:{line:range.endLineNumber-1,character:range.endColumn-1}}});return {hints:(hints||[]).map(h=>({position:{lineNumber:h.position.line+1,column:h.position.character+1},label:typeof h.label==='string'?h.label:h.label.map(l=>l.value).join(''),kind:h.kind,paddingLeft:h.paddingLeft,paddingRight:h.paddingRight,tooltip:this.markdown(h.tooltip)})),dispose(){}};}}));
      add(m.languages.registerDocumentHighlightProvider(language,{provideDocumentHighlights:async(model,p)=>(await this.request('textDocument/documentHighlight',this.params(model,p))||[]).map(v=>({range:this.range(v.range),kind:v.kind}))}));
      add(m.languages.registerFoldingRangeProvider(language,{provideFoldingRanges:async model=>(await this.request('textDocument/foldingRange',this.params(model))||[]).map(v=>({start:v.startLine+1,end:v.endLine+1,kind:m.languages.FoldingRangeKind.fromValue(v.kind||'region')}))}));
    }
  }
  dispose(){for(const d of this.disposables)d.dispose();this.socket?.close();}
}
