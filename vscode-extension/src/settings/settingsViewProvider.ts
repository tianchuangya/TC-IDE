/** TC-IDE's settings live in a native VS Code side view, not a fake editor. */
import * as vscode from 'vscode';

export class SettingsViewProvider implements vscode.WebviewViewProvider {
  static readonly viewType = 'tcIde.settingsView';
  private view: vscode.WebviewView | undefined;
  constructor(private readonly context: vscode.ExtensionContext) {}

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    view.webview.options = { enableScripts: true };
    view.webview.html = this.html();
    view.webview.onDidReceiveMessage((message: unknown) => { void this.onMessage(message); });
  }
  reveal(): void { void vscode.commands.executeCommand('tcIde.settingsView.focus'); }

  private async onMessage(message: unknown): Promise<void> {
    if (!message || typeof message !== 'object') return;
    const data = message as { type?: string; key?: string; value?: unknown };
    if (data.type === 'state') { this.sendState(); return; }
    if (data.type === 'chooseBackground') {
      const picked = await vscode.window.showOpenDialog({ canSelectMany: false, filters: { '图片': ['png', 'jpg', 'jpeg', 'webp'] }, title: '选择 TC-IDE 页面背景' });
      const uri = picked?.[0];
      if (uri) await this.update('appearance.backgroundImage', uri.fsPath);
      this.sendState(); return;
    }
    if (data.type === 'set' && typeof data.key === 'string') {
      const allowed = new Set(['editor.typingEffect', 'editor.commentBubbles', 'editor.commentBubbleStyle', 'appearance.backgroundOpacity']);
      if (allowed.has(data.key)) await this.update(data.key, data.value);
      this.sendState();
    }
  }
  private async update(key: string, value: unknown): Promise<void> {
    await vscode.workspace.getConfiguration('tcIde').update(key, value, vscode.ConfigurationTarget.Global);
  }
  private sendState(): void {
    const config = vscode.workspace.getConfiguration('tcIde');
    void this.view?.webview.postMessage({ type: 'state', value: {
      typingEffect: config.get('editor.typingEffect', 'pulse'),
      commentBubbles: config.get('editor.commentBubbles', true),
      commentBubbleStyle: config.get('editor.commentBubbleStyle', 'soft'),
      backgroundOpacity: config.get('appearance.backgroundOpacity', 0.12),
      backgroundImage: config.get('appearance.backgroundImage', ''),
    } });
  }
  private html(): string {
    const nonce = Math.random().toString(36).slice(2);
    return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}'"><style>
body{font:13px var(--vscode-font-family);color:var(--vscode-foreground);padding:12px;background:var(--vscode-sideBar-background)}h2{font-size:14px;margin:4px 0 12px}.group{border-top:1px solid var(--vscode-widget-border);padding:12px 0;display:grid;gap:10px}label{display:flex;justify-content:space-between;align-items:center;gap:10px}.hint{color:var(--vscode-descriptionForeground);font-size:11px;line-height:16px}select,input,button{font:inherit}select,input{background:var(--vscode-input-background);color:var(--vscode-input-foreground);border:1px solid var(--vscode-input-border);border-radius:4px;padding:5px}button{background:var(--vscode-button-background);color:var(--vscode-button-foreground);border:0;border-radius:4px;padding:6px 9px;cursor:pointer}button:hover{background:var(--vscode-button-hoverBackground)}.row{display:flex;gap:8px}.row input{width:70px}</style></head><body><h2>TC‑IDE 外观与动效</h2><section class="group"><b>编辑器微动效</b><label>输入 / 删除 <select id="typing"><option value="pulse">脉冲描边</option><option value="glow">柔和光晕</option><option value="none">关闭</option></select></label><span class="hint">使用 VS Code 装饰层，不改变代码内容或输入性能。</span></section><section class="group"><b>注释气泡</b><label>显示气泡 <input id="bubble" type="checkbox"></label><label>气泡风格 <select id="bubbleStyle"><option value="soft">柔和填充</option><option value="outline">描边</option><option value="minimal">极简</option></select></label></section><section class="group"><b>TC‑IDE 页面背景</b><button id="choose">选择背景图片</button><label>不透明度 <span class="row"><input id="opacity" type="range" min="0" max="1" step="0.01"><output id="opacityOut"></output></span></label><span class="hint">背景仅应用于 TC‑IDE 的关系图和设置页面，保持原生 VS Code 编辑器稳定。</span></section><script nonce="${nonce}">const v=acquireVsCodeApi();const $=id=>document.getElementById(id);const send=(key,value)=>v.postMessage({type:'set',key,value});$('typing').onchange=e=>send('editor.typingEffect',e.target.value);$('bubble').onchange=e=>send('editor.commentBubbles',e.target.checked);$('bubbleStyle').onchange=e=>send('editor.commentBubbleStyle',e.target.value);$('choose').onclick=()=>v.postMessage({type:'chooseBackground'});$('opacity').oninput=e=>{ $('opacityOut').value=e.target.value; send('appearance.backgroundOpacity',Number(e.target.value));};addEventListener('message',e=>{if(e.data.type!=='state')return;const s=e.data.value;$('typing').value=s.typingEffect;$('bubble').checked=s.commentBubbles;$('bubbleStyle').value=s.commentBubbleStyle;$('opacity').value=s.backgroundOpacity;$('opacityOut').value=s.backgroundOpacity;});v.postMessage({type:'state'});</script></body></html>`;
  }
}
