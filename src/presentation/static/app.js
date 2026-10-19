'use strict';
(async()=>{
  const $=id=>document.getElementById(id);
  const make=(tag,cls,text)=>{const e=document.createElement(tag);if(cls)e.className=cls;if(text!==undefined)e.textContent=text;return e;};
  const exporting=new URLSearchParams(location.search).get('export')==='1';if(exporting)document.body.classList.add('export');
  let data,evidence,nativeRecord,current=0,step=0,activeDemo='',mode='interpreter',running=false,available=false,terminalMode=false,editorReady=null,messageTimer,artifactUrl;
  window.PrimPresentation={ready:false,show,get step(){return step;},get data(){return data;},get current(){return data?.slides[current]?.id;},get evidence(){return evidence;},get nativeRecord(){return nativeRecord;}};
  const editorLoaded=new Promise((resolve,reject)=>{if(window.PrimEditor)return resolve(window.PrimEditor);const timer=setTimeout(()=>reject(new Error('编辑器资源未就绪，请先构建 web 前端')),20000);window.addEventListener('prim-editor-ready',()=>{clearTimeout(timer);resolve(window.PrimEditor);},{once:true});});
  const message=text=>{$('message').textContent=text;$('message').hidden=false;clearTimeout(messageTimer);messageTimer=setTimeout(()=>$('message').hidden=true,5000);};
  const ms=value=>value===0?'< 1':Number(value).toLocaleString('en-US',{maximumFractionDigits:value<10?2:1});
  const store={get:k=>{try{return localStorage.getItem(k);}catch{return null;}},set:(k,v)=>{try{localStorage.setItem(k,v);}catch{}},del:k=>{try{localStorage.removeItem(k);}catch{}}};
  async function api(path,body){const response=await fetch(path,{method:'POST',headers:{'Content-Type':'application/json','X-Prim-Token':document.querySelector('meta[name="prim-token"]').content},body:JSON.stringify(body)});const value=await response.json();if(!response.ok)throw new Error(value.error||`HTTP ${response.status}`);return value;}
  const demoById=id=>data.demos.find(d=>d.id===id);

  /* ---------- slide content ---------- */
  const at=(el,n)=>{if(n)el.dataset.step=n;return el;};
  function bullet(text){
    const p=make('p','bullet'),m=text.match(/^(.+?)：(.+)$/);
    if(m&&/[[\](){}=&$@.]/.test(m[1])){p.append(make('code','',m[1]),m[2]);}else p.textContent=text;
    return p;
  }
  function block(b){
    if(b.kind==='table'){const table=make('table');b.rows.forEach((row,i)=>{const tr=make('tr');row.forEach(cell=>tr.append(make(i?'td':'th','',cell)));table.append(tr);});return at(table,b.step);}
    if(b.kind==='bullet')return at(bullet(b.text),b.step);
    if(b.kind==='output')return at(make('p','output',b.text),b.step);
    if(b.kind==='emphasis')return at(make('p','emphasis',b.text),b.step);
    return at(make('p',b.step?'takeaway':'muted',b.text),b.step);
  }
  const copyOf=(blocks,cls='copy')=>{const copy=make('div',cls);for(const b of blocks)if(b.kind!=='code')copy.append(block(b));return copy;};
  async function codePanel(b){const panel=at(make('section','code-panel'),b.step);panel.append(make('div','code-title',b.label));const code=make('div','code-text');code.setAttribute('aria-label',b.label+'代码');code.innerHTML=await window.PrimEditor.colorize(b.text,b.language==='text'?'plaintext':b.language);panel.append(code);return panel;}
  function figure(slide){const key=slide.options.diagram;if(!key)return null;const area=make('div','figure-area');area.innerHTML=window.PrimDiagrams.render(key,slide.fields['设计说明']);return area.firstChild?area:null;}

  function metricCard(scenario){
    const card=make('section','evidence-card');card.dataset.evidence=scenario;
    if(!evidence?.available){card.append(make('h2','','等待真实测量'),make('p','evidence-meta',evidence?.message||'尚无测量记录。'));return card;}
    const stale=evidence.source_matches===false,modes=evidence.results?.[scenario];
    if(!modes?.interpreter||!modes?.jit){card.append(make('h2','','等待重新测量'),make('p','evidence-meta','记录格式与当前场景不符，请重新测量。'));return card;}
    const size=evidence.numerical_workload?.size;
    card.append(make('span',`proof-badge${stale?' stale':''}`,stale?'源码已变化 · 历史测量':'结果逐项校验一致'));
    card.append(make('h2','',scenario==='jit'?`${size}×${size} 矩阵乘法`:'订单汇总'));
    const names={interpreter:'解释执行',jit:'JIT'};
    const table=make('table','evidence-table');const head=make('tr');['','首次 ms','热调用 ms','进程 ms'].forEach(v=>head.append(make('th','',v)));table.append(head);
    for(const key of ['interpreter','jit']){const v=modes[key],row=make('tr');[names[key],ms(v.first_ms),ms(v.warm_per_call_ms),ms(v.process_ms)].forEach(x=>row.append(make('td','',x)));table.append(row);}
    card.append(table);
    const bars=make('div','bars'),top=Math.max(modes.interpreter.warm_per_call_ms,modes.jit.warm_per_call_ms);
    for(const key of ['interpreter','jit']){const v=modes[key],track=make('div','track'),fill=make('div',`fill${key==='jit'?' accent':''}`);fill.style.width=`${Math.max(1,v.warm_per_call_ms/top*100)}%`;track.append(fill);bars.append(make('span','',names[key]),track,make('b','',`${ms(v.warm_per_call_ms)} ms`));}
    card.append(bars);
    const a=modes.interpreter.warm_per_call_ms,b=modes.jit.warm_per_call_ms,ratio=b>0?a/b:0;
    const verdict=scenario==='jit'
      ?(b>0?`热调用快 ${ratio.toFixed(ratio>=10?0:2)} 倍；JIT 首次调用另含编译 ${ms(modes.jit.compile_ms)} ms。`:'JIT 热调用低于 1 ms 计时分辨率，不计算倍数。')
      :`业务结果一致。${b>=a?'这里以字符串与数组操作为主，多走运行时 helper，JIT 没有带来收益，如实列出。':`热调用：解释 / JIT = ${ratio.toFixed(2)}×。`}`;
    card.append(make('p',`evidence-verdict${scenario==='orders'?' caution':''}`,verdict));
    const date=new Date(evidence.generated_at).toLocaleString('zh-CN',{hour12:false});
    const workload=scenario==='orders'?`${evidence.dataset.rows} 条合成订单 · 有效 ${evidence.expected.accepted} 条`:`每次调用 ${Number(evidence.numerical_workload?.multiply_adds_per_call||0).toLocaleString('en-US')} 次乘加 · 结果与独立参考一致`;
    card.append(make('p','evidence-meta',`${evidence.protocol.independent_processes_per_mode} 个独立进程 / 模式 · 每进程热调用 ${evidence.protocol.warm_calls_per_process} 次 · 中位数 · 计时精度 1 ms\n${workload}\n${evidence.machine.cpu} · ${date}${evidence.runtime_matches===false?'\n当前执行程序与记录不同；表中为标明版本的历史测量。':''}`));
    const actions=make('div','evidence-action');const rerun=make('button','ghost small','重新测量');rerun.addEventListener('click',()=>remeasure(rerun));actions.append(rerun);card.append(actions);
    return card;
  }
  async function remeasure(button){button.disabled=true;button.textContent='测量中…';try{await api('/api/benchmark',{});evidence=await fetch('/api/evidence').then(r=>r.json());document.querySelectorAll('[data-evidence]').forEach(old=>old.replaceWith(metricCard(old.dataset.evidence)));message('实测已更新，结果校验通过。');}catch(error){message(error.message);button.disabled=false;button.textContent='重新测量';}}
  function nativeCard(){
    const card=make('section','evidence-card');
    if(!nativeRecord?.available){card.append(make('h2','','尚无机器码记录'),make('p','evidence-meta',nativeRecord?.message||'下一页可现场读取。'));return card;}
    const m=nativeRecord.metadata;
    card.append(make('span',`proof-badge${nativeRecord.source_matches===false?' stale':''}`,`${m.origin} · ${m.size} bytes`));
    card.append(make('h2','',`${m.symbol} @ ${m.address}`));
    const pre=make('pre','native-excerpt');pre.textContent=nativeRecord.excerpt.map(r=>`${r.address.slice(-6)}  ${r.bytes.padEnd(21)} ${r.instruction.replace(/\t/,' ')}`).join('\n');card.append(pre);
    card.append(make('p','evidence-meta',`${nativeRecord.runtime} · ${m.triple}\ncompile 后 native_calls = ${m.calls_before}，验证调用后 = ${m.calls_after}；入口地址受 ASLR 影响。`));
    return card;
  }
  async function fillPipeline(grid){
    const demo=demoById('pipeline');
    const fail=text=>grid.querySelectorAll('.pipe-text').forEach(pre=>{pre.replaceChildren(make('span','pipe-missing',text));});
    if(!available||!demo)return fail('需要 Prim 执行程序；本页内容由 std.inspect 现场生成。');
    let result;try{result=await api('/api/run',{code:demo.code,mode:'jit'});}catch(error){return fail(error.message);}
    if(result.exit_code!==0)return fail(result.stderr.slice(-400));
    const sections={};let name='';for(const line of result.stdout.split('\n')){const m=line.match(/^=== (.+) ===$/);if(m){name=m[1];sections[name]=[];}else if(name)sections[name].push(line);}
    const trim=a=>{a=[...(a||[])];while(a.length&&!a.at(-1).trim())a.pop();return a;};
    const pre=key=>grid.querySelector(`[data-stage="${key}"] .pipe-text`),note=(key,text)=>{grid.querySelector(`[data-stage="${key}"] em`).textContent=text;};
    const source=pre('源码');source.classList.add('big');source.replaceChildren(make('span','',trim(sections['源码']).join('\n')),make('span','dim',`square(7) → ${trim(sections['调用']).join(' ')}`));
    const ast=trim(sections['AST']);pre('AST').textContent=ast.join('\n');note('AST',`${ast.length} 个节点`);
    const ir=trim(sections['Core IR']);pre('Core IR').textContent=ir.join('\n');note('Core IR',ir[0]?.replace(/^function #\d+,\s*/,'')||'');
    const asm=trim(sections['机器码']).map(line=>line.match(/^([0-9a-f]+)\s+((?:[0-9a-f]{2}\s)+)\s*(.*)$/i)).filter(Boolean);
    const hot=Math.max(0,asm.findIndex(m=>/\bimul\b/.test(m[3])));const bytes=asm.reduce((n,m)=>n+m[2].trim().split(/\s+/).length,0);
    pre('机器码').replaceChildren(...asm.slice(Math.max(0,hot-4),hot+5).map(m=>make('span',/\b(imul|jno)\b/.test(m[3])?'hot':'',`${m[1].slice(-4)}  ${m[3].replace(/\t/,' ')}`)));
    note('机器码',`${asm.length} 条指令 · ${bytes} 字节 · 截取`);
  }
  /* Static stand-in for a live page: shown in export, and until the editor mounts. */
  async function liveStatic(demo){
    const grid=make('div','live-static');
    grid.append(await codePanel({text:demo.code,language:'prim',label:demo.tab,step:0}));
    const out=make('section','code-panel output-panel');out.append(make('div','code-title','执行结果'));const pre=make('pre','code-text');out.append(pre);grid.append(out);
    if(demo.kind!=='code')pre.textContent=`终端会话 · ${demo.description}`;
    else if(exporting&&available){try{const r=await api('/api/run',{code:demo.code,mode:demo.mode||'interpreter'});pre.textContent=r.stdout.trimEnd();}catch(error){pre.textContent=error.message;}}
    else pre.textContent='按“运行”或 Ctrl+Enter 执行';
    return grid;
  }

  async function render(slide){
    const node=make('section',`slide layout-${slide.layout}`);node.dataset.slide=slide.id;node.id=`slide-${slide.id}`;node.hidden=true;node.setAttribute('aria-label',`${slide.id} ${slide.title}`);
    let steps=slide.steps;
    const body=make('div','slide-body'),codes=slide.blocks.filter(b=>b.kind==='code'),texts=slide.blocks.filter(b=>b.kind!=='code');
    if(slide.layout!=='cover'){const head=make('header','slide-head');head.append(make('p','eyebrow',slide.chapter||' '),make('h1','',slide.title));node.append(head);}
    switch(slide.layout){
      case 'cover':{
        const left=make('div');left.append(make('p','cover-eyebrow',data.presenter.course||data.metadata.presentation_type),make('p','cover-title',texts[0].text),make('p','cover-subtitle',texts[1].text),make('p','cover-question',texts[2].text));
        const art=make('div','cover-art'),img=make('img');img.src='/static/logo.svg';img.alt='Prim';art.append(img);body.append(left,art);break;}
      case 'question':{
        const lead=texts.find(b=>b.kind==='emphasis'),ask=texts.filter(b=>b.kind==='emphasis').at(-1);
        body.append(at(make('p','emphasis lead',lead.text),lead.step),await codePanel(codes[0]));
        const seq=make('div','sequence');texts.filter(b=>b.kind==='bullet').forEach((b,i)=>{if(i)seq.append(make('span','arrow'));seq.append(at(make('span','chip',b.text),b.step));});
        body.append(seq,at(make('p','ask',ask.text),ask.step));break;}
      case 'pair':{
        const lead=texts[0];body.append(at(make('p','emphasis lead',lead.text),lead.step));
        const pair=make('div','code-pair');for(const c of codes)pair.append(await codePanel(c));body.append(pair);
        for(const b of texts.slice(1))body.append(at(make('p','takeaway',b.text),b.step));break;}
      case 'cards':{
        body.className='slide-body cards-body';const lead=texts[0];body.append(at(make('p','emphasis lead',lead.text),lead.step));
        const cards=make('div','cards');texts.filter(b=>b.kind==='bullet').forEach((b,i)=>{const [title,text]=b.text.split('｜');const card=at(make('section','concept-card'),b.step);card.append(make('span','card-index',`0${i+1}`),make('h2','',title),make('p','',text||''));cards.append(card);});body.append(cards);
        for(const b of texts.slice(1).filter(b=>b.kind!=='bullet'))body.append(at(make('p',b.kind==='emphasis'?'cards-question':'cards-note',b.text),b.step));break;}
      case 'code':{
        node.classList.add('code-slide');body.append(await codePanel(codes[0]));
        const copy=make('div','copy');for(const b of texts)copy.append(block(b));
        const fig=figure(slide);if(fig)copy.append(fig);body.append(copy);break;}
      case 'live':{
        const demo=demoById(slide.options.demo);if(!demo)throw new Error(`${slide.id} 引用了不存在的示例 ${slide.options.demo}`);
        body.append(make('p','live-lead',texts[0]?.text||demo.description));
        const host=make('div','live-host');host.dataset.demo=demo.id;host.append(await liveStatic(demo));body.append(host);break;}
      case 'pipeline':{
        const lead=texts[0];body.append(at(make('p','emphasis lead',lead.text),lead.step));
        const grid=make('div','pipe-grid');texts.filter(b=>b.kind==='bullet').forEach((b,i)=>{const [title,api]=b.text.split('｜');const stage=at(make('section','pipe-stage'),i);stage.dataset.stage=title;const headRow=make('div','pipe-head');headRow.append(make('strong','',title),make('span','',api),make('em'));stage.append(headRow,make('pre','pipe-text','…'));grid.append(stage);});
        body.append(grid);const shift=grid.children.length-1;
        for(const b of texts.slice(1).filter(b=>b.kind!=='bullet'))body.append(at(make('p','takeaway',b.text),b.step+shift));
        steps+=shift;await fillPipeline(grid);break;}
      case 'bench':case 'orders':node.classList.add('evidence-slide');body.append(copyOf(texts),metricCard(slide.layout==='bench'?'jit':'orders'));break;
      case 'snake':{
        node.classList.add('evidence-slide');body.append(copyOf(texts));
        const record=make('section','evidence-card');record.append(make('h2','','一次真实 PTY 运行的画面'));
        const img=make('img','game-record');img.src='/evidence/snake.png';img.alt='Prim 贪吃蛇终端运行记录';img.onerror=()=>img.replaceWith(make('div','game-missing','尚未保存运行画面。\n下一页现场运行 snake.pr。'));
        record.append(img,make('p','evidence-meta','记录画面；下一页现场运行。'));body.append(record);break;}
      case 'protocol':{
        node.classList.add('evidence-slide');body.append(copyOf(texts));const card=make('section','evidence-card');card.append(make('h2','','测量记录与版本'));
        const size=evidence?.numerical_workload?.size??'?';
        card.append(make('p','evidence-meta',evidence?.available?`${evidence.runtime}\n${evidence.machine.cpu}\n矩阵：${size}×${size} · 订单：${evidence.dataset.rows} 条，seed=${evidence.dataset.seed}\n采样：每模式 ${evidence.protocol.independent_processes_per_mode} 个独立进程\n运行时 SHA256：${evidence.runtime_sha256.slice(0,16)}…`:'尚未完成测量。'));body.append(card);break;}
      case 'native':node.classList.add('evidence-slide');body.append(copyOf(texts),nativeCard());break;
      case 'table':node.classList.add('wide-table');body.append(copyOf(texts));break;
      default:{node.classList.add('text-slide');body.append(copyOf(texts));const fig=figure(slide);if(fig)body.append(fig);}
    }
    node.dataset.steps=steps;node.append(body);
    const foot=make('footer','slide-foot'),who=Object.values(data.presenter).filter(Boolean).join(' · ');
    foot.append(make('span','',slide.id==='01'&&who?who:data.metadata.title.replace('：',' · ')),make('span','page-id',`${slide.id} / ${String(data.slides.length).padStart(2,'0')}`));node.append(foot);
    return node;
  }
  async function build(){
    $('deck').replaceChildren();$('overview-grid').replaceChildren();
    let chapter=null;
    for(const slide of data.slides){
      $('deck').append(await render(slide));
      if(slide.chapter!==chapter){chapter=slide.chapter;$('overview-grid').append(make('p','overview-chapter',chapter||'开场'));}
      const entry=make('button',`overview-card${slide.layout==='live'?' live':''}`);entry.dataset.page=slide.id;entry.append(make('span','',slide.id),make('strong','',slide.title));entry.addEventListener('click',()=>{$('overview').close();show(slide.id);});$('overview-grid').append(entry);
    }
    $('brand-chapter').textContent=data.metadata.title.split('：').at(-1);
  }

  /* ---------- navigation ---------- */
  const active=()=>document.querySelector('.slide.active');
  function applySteps(){const node=active();if(!node)return;node.querySelectorAll('[data-step]').forEach(el=>el.classList.toggle('pending',Number(el.dataset.step)>step));}
  function show(id,options={}){
    if(!data)return;const index=typeof id==='number'?Math.max(0,Math.min(data.slides.length-1,id)):data.slides.findIndex(s=>s.id===id);if(index<0)return;
    if(index!==current)unmountRig();
    current=index;
    document.querySelectorAll('.slide').forEach((s,i)=>{s.hidden=i!==current;s.classList.toggle('active',i===current);});
    const slide=data.slides[current],node=active();step=options.step==='all'?Number(node.dataset.steps):0;applySteps();
    $('counter').textContent=`${slide.id} / ${data.slides.length}`;$('prev').disabled=current===0;$('next').disabled=current===data.slides.length-1;$('progress').style.width=`${(current+1)/data.slides.length*100}%`;
    $('chapter-label').textContent=slide.chapter||'';
    document.querySelectorAll('.overview-card').forEach(e=>e.classList.toggle('current',e.dataset.page===slide.id));
    if(slide.layout==='live'&&!exporting)mountRig(node.querySelector('.live-host')).catch(e=>message(e.message));
    if(location.hash!==`#${slide.id}`)history[options.replace?'replaceState':'pushState'](null,'',`#${slide.id}`);document.title=`${slide.id} · ${slide.title} — Prim`;if(options.focus)$('stage').focus({preventScroll:true});
  }
  function next(){if(step<Number(active()?.dataset.steps||0)){step++;applySteps();}else if(current<data.slides.length-1)show(current+1);}
  function prev(){if(step>0){step--;applySteps();}else if(current>0)show(current-1,{step:'all'});}
  async function fullscreen(){try{if(document.fullscreenElement)await document.exitFullscreen();else await document.documentElement.requestFullscreen();}catch{message('请使用浏览器的全屏功能。');}}

  /* ---------- code pages: one editor + terminal, moved into the active live slide ---------- */
  async function status(){try{const r=await fetch('/api/status').then(r=>r.json());available=r.available;$('runtime-status').textContent=available?r.version:r.message;$('runtime-status').classList.toggle('unavailable',!available);$('run').disabled=running||!available;}catch{available=false;$('run').disabled=true;$('runtime-status').textContent='执行服务无法连接';}}
  const demo=()=>demoById(activeDemo);
  function setMode(value){mode=value;document.querySelectorAll('[data-mode]').forEach(b=>b.setAttribute('aria-checked',String(b.dataset.mode===value)));}
  function modeLock(){document.querySelectorAll('[data-mode]').forEach(b=>b.disabled=running);}
  function initEditor(){
    editorReady??=(async()=>{
      await editorLoaded;window.PrimEditor.onStatus(text=>$('language-status').textContent=text);
      await window.PrimEditor.initEditor($('editor'),data.workspace_uri,text=>{if(activeDemo&&window.PrimEditor.getLanguage()!=='prim-interface')store.set(`prim-demo-v3-${activeDemo}`,text);});
      await window.PrimEditor.initTerminal($('terminal'));
    })();
    return editorReady;
  }
  function selectDemo(id){
    const d=demoById(id);activeDemo=id;
    window.PrimEditor.setCode(id,store.get(`prim-demo-v3-${id}`)??d.code,data.workspace_uri,'prim');
    $('rig').dataset.kind=d.kind;$('rig').dataset.demo=id;$('editor-name').textContent=d.tab;fitRig();
    window.PrimEditor.clear();$('run-status').textContent='尚未运行';$('run-status').className='';$('artifact').hidden=true;setMode(d.mode||'interpreter');modeLock();
  }
  // The snake board needs at least 52×21 cells, so its terminal uses a smaller font.
  function fitRig(){const deck=$('deck').getBoundingClientRect().width,px=Math.round(Math.max(12,Math.min(22,deck*.0102)));window.PrimEditor.setFontSize(px,activeDemo==='snake'?Math.round(px*.78):px-1);window.PrimEditor.layout();}
  async function mountRig(host){
    await initEditor();
    if(!host||!active()?.contains(host)||host.contains($('rig')))return;
    host.append($('rig'));host.classList.add('mounted');selectDemo(host.dataset.demo);
    requestAnimationFrame(fitRig);await status();
  }
  function unmountRig(){
    const host=$('rig').parentElement;if(!host?.classList.contains('live-host'))return;
    if(running)stop();host.classList.remove('mounted');$('rig-pool').append($('rig'));activeDemo='';$('stage').focus({preventScroll:true});
  }
  function busy(value){running=value;$('run').disabled=value||!available;$('stop').disabled=!value;$('reset-demo').disabled=value;modeLock();}
  function exitStatus(text,ok){$('run-status').textContent=text;$('run-status').className=ok?'success':'error';}
  async function run(){
    if(running||!available||!activeDemo)return;if(window.PrimEditor.getLanguage()==='prim-interface'){message('这是 builtin 接口定义，请先回到示例代码。');return;}
    const code=window.PrimEditor.getValue(),d=demo();busy(true);$('artifact').hidden=true;exitStatus('运行中…',true);
    terminalMode=d.kind==='terminal'&&mode!=='check';
    try{
      if(terminalMode){await window.PrimEditor.startTerminal({code,mode,scenario:d.id},event=>{
        if(event.kind==='started'){$('runtime-status').textContent=event.runtime;exitStatus('实时会话 · 按键已连接',true);}
        if(event.kind==='artifact'){if(artifactUrl)URL.revokeObjectURL(artifactUrl);artifactUrl=URL.createObjectURL(new Blob([event.text],{type:'text/plain;charset=utf-8'}));$('artifact').href=artifactUrl;$('artifact').hidden=false;}
        if(event.kind==='exit'){exitStatus(`退出 ${event.code} · ${event.reason}`,event.code===0);busy(false);}
        if(event.kind==='error'){exitStatus(event.message,false);busy(false);}
        if(event.kind==='closed'&&running){exitStatus('终端会话已断开',false);busy(false);}
      });}
      else{const result=await api('/api/run',{code,mode});window.PrimEditor.writeResult(result);exitStatus(`${result.reason||'退出'} ${result.exit_code} · ${result.elapsed_ms} ms`,result.exit_code===0&&!result.reason);busy(false);}
    }catch(error){exitStatus(error.message,false);busy(false);}
  }
  async function stop(){if(terminalMode)window.PrimEditor.stop();else try{await api('/api/stop',{});}catch(error){message(error.message);}}

  /* ---------- wiring ---------- */
  $('prev').onclick=()=>{prev();$('stage').focus({preventScroll:true});};$('next').onclick=()=>{next();$('stage').focus({preventScroll:true});};
  $('overview-button').onclick=()=>$('overview').showModal();$('overview-close').onclick=()=>$('overview').close();$('fullscreen-button').onclick=fullscreen;
  document.querySelectorAll('[data-mode]').forEach(b=>b.addEventListener('click',()=>{if(!b.disabled)setMode(b.dataset.mode);}));
  $('run').onclick=run;$('stop').onclick=stop;window.addEventListener('prim-run',run);
  $('reset-demo').onclick=()=>{store.del(`prim-demo-v3-${activeDemo}`);window.PrimEditor.setCode(activeDemo,demo().code,data.workspace_uri,'prim');};
  $('editor-return').onclick=()=>selectDemo(activeDemo);
  window.addEventListener('prim-model',e=>{$('editor-return').hidden=e.detail.language!=='prim-interface';});
  window.addEventListener('resize',()=>{if(activeDemo)fitRig();});
  window.addEventListener('hashchange',()=>show(location.hash.slice(1),{replace:true}));
  // Clickers send PageUp/PageDown; keep them working even while the editor or terminal has focus.
  document.addEventListener('keydown',e=>{
    if(!data||document.querySelector('dialog[open]')||!['PageDown','PageUp'].includes(e.key))return;
    e.preventDefault();e.stopPropagation();if(e.key==='PageDown')next();else prev();
  },true);
  document.addEventListener('keydown',e=>{
    if(!data||document.querySelector('dialog[open]')||e.ctrlKey||e.metaKey||e.altKey)return;
    if(/INPUT|TEXTAREA|SELECT|BUTTON/.test(e.target.tagName)||e.target.closest?.('.rig'))return;
    const key=e.key.toLowerCase();
    if(['ArrowRight',' '].includes(e.key)){e.preventDefault();next();}
    else if(e.key==='ArrowLeft'){e.preventDefault();prev();}
    else if(e.key==='Home')show(0);else if(e.key==='End')show(data.slides.length-1);
    else if(key==='o')$('overview').showModal();else if(key==='f')fullscreen();
  });
  try{
    [data,evidence,nativeRecord]=await Promise.all([fetch('/api/deck').then(async r=>{const v=await r.json();if(!r.ok)throw new Error(v.error);return v;}),fetch('/api/evidence').then(r=>r.json()),fetch('/api/native').then(r=>r.json())]);
    await editorLoaded;await window.PrimEditor.ready;await Promise.all(['18px "JetBrains Mono"','24px "Source Serif 4"','24px "Prim Serif SC"','16px Inter'].map(f=>document.fonts.load(f,'Prim 闭包')));
    await status();await build();
    if(!exporting)initEditor().catch(e=>message(e.message));
    current=-1;show(data.slides.some(s=>s.id===location.hash.slice(1))?location.hash.slice(1):0,{replace:true});
    await document.fonts.ready;window.PrimPresentation.ready=true;
  }catch(error){$('deck').replaceChildren(make('div','loading',`展示加载失败：${error.message}`));}
})();
