/* Structural diagrams of the language model; runtime results live in the workbench. */
(() => {
  const esc = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  let suffix = '';
  const t = (x,y,s,cls='label',anchor='middle') => `<text x="${x}" y="${y}" class="${cls}" text-anchor="${anchor}">${esc(s)}</text>`;
  const box = (x,y,w,h,label,cls='clay') => `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="12" class="node ${cls}"/>${t(x+w/2,y+h/2+7,label)}`;
  const edge = (x1,y1,x2,y2) => `<path d="M${x1},${y1} L${x2},${y2}" class="edge" marker-end="url(#arrow-${suffix})"/>`;
  const path = d => `<path d="${d}" class="edge" fill="none" marker-end="url(#arrow-${suffix})"/>`;
  const region = (x,y,w,h,label) => `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="18" class="region"/>${t(x+20,y+31,label,'small','start')}`;
  const badge = (x,y,w,s) => box(x,y,w,40,s,'neutral');
  const env = (x,y,name,code) => region(x,y,260,240,name)+box(x+24,y+57,212,48,'环境','olive')+box(x+24,y+121,96,48,'槽','kraft')+box(x+140,y+121,96,48,'槽','kraft')+(code?box(x+24,y+187,212,38,'调用时执行','clay'):t(x+130,y+211,'构造时已执行','small'));
  const drawings = {
    'closure-kinds': () => env(28,40,'$[…](…) { … }',true)+env(352,40,'@{ … }',false)+t(320,330,'同样的环境与槽，区别只在 closuring 何时执行','caption'),
    'slots': () => {
      const state=(x,n,label)=>region(x,20,280,300,label)+edge(x+84,99,x+174,132)+edge(x+84,187,x+174,153)+edge(x+84,268,x+174,268)+box(x+20,74,64,48,'a','neutral')+box(x+20,163,64,48,'c','neutral')+box(x+20,243,64,48,'b','neutral')+box(x+174,114,78,58,String(n),'kraft')+box(x+174,240,78,58,'1','kraft');
      return state(18,1,'写入之前')+state(342,2,'a = 2 之后')+t(320,350,'a 与 c 同槽；b 有自己的槽','caption');
    },
    'capture': () => region(20,15,600,310,'构造时 amount = 200，随后写为 300')+edge(220,181,395,105)+edge(395,260,220,200)+box(50,150,170,75,'外部槽：300','kraft')+box(395,70,180,70,'私有槽：200','kraft')+box(395,230,180,70,'共享该槽','olive')+t(320,114,'[amount] 复制','small')+t(305,277,'[&amount] 共享','small')+t(320,355,'捕获关系在构造时建立','caption'),
    'accumulator': () => region(18,15,420,315,'orders')+region(460,15,165,315,'另一个实例')+path('M164,107 L282,177')+path('M164,202 L282,197')+path('M164,293 L282,217')+box(35,81,130,50,'total','neutral')+box(35,177,130,50,'add','clay')+box(35,267,130,50,'read','clay')+box(282,165,130,70,'500','kraft')+box(482,165,120,70,'0','kraft')+t(320,358,'add 与 read 共享 total 的槽','caption'),
    'gc': () => region(20,15,284,316,'仍可达：保留')+region(336,15,284,316,'不可达的环：回收')+edge(162,115,162,177)+box(54,70,215,53,'活动帧 / 环境','olive')+box(54,182,215,53,'闭包与捕获槽','kraft')+box(368,115,98,62,'对象 A','neutral')+box(490,220,98,62,'对象 B','neutral')+edge(456,164,509,220)+path('M490,252 L375,252 L375,180')+t(478,85,'没有根指向','small')+t(162,287,'根可以追踪到','small')+t(320,360,'槽与闭包池：标记清扫','caption'),
    'jit-shared': () => edge(176,93,319,208)+edge(480,93,337,208)+box(38,30,250,66,'add · 解释执行','olive')+box(354,30,250,66,'native · JIT 入口','clay')+box(228,200,185,65,'total 槽','kraft')+badge(28,302,158,'显式标记')+edge(190,322,228,322)+badge(239,302,168,'首次调用编译')+edge(411,322,447,322)+badge(457,302,158,'之后复用'),
    'tooling': () => box(35,45,200,70,'Monaco 编辑器','clay')+edge(240,80,400,80)+box(405,45,200,70,'Prim LSP','olive')+t(320,54,'WebSocket','small')+box(35,210,200,70,'xterm.js 终端','clay')+edge(240,245,400,245)+box(405,210,200,70,'PTY / Prim','kraft')+t(320,220,'双向输入输出','small')+t(320,340,'编辑服务与程序执行分离','caption'),
  };
  window.PrimDiagrams = {render(id, description) {
    suffix = id;
    const draw = drawings[id];
    if (!draw) return '';
    return `<figure class="diagram"><svg viewBox="0 0 640 370" role="img" aria-label="${esc(description)}"><defs><marker id="arrow-${id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 Z" class="arrow-head"/></marker></defs>${draw()}</svg></figure>`;
  }};
})();
