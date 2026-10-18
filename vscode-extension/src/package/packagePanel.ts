/** A visual status panel driven only by a real task outcome. */
import * as vscode from 'vscode';

type Phase = 'preparing' | 'absorbing' | 'building' | 'emitting' | 'success' | 'failure';

export class PackagePanel {
  private panel: vscode.WebviewPanel | undefined;
  show(phase: Phase, text: string): void {
    if (!this.panel) {
      this.panel = vscode.window.createWebviewPanel('tcIde.package', 'TC-IDE 打包', vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true });
      this.panel.onDidDispose(() => { this.panel = undefined; });
      this.panel.webview.html = html();
    }
    this.panel.reveal(vscode.ViewColumn.Beside, true);
    void this.panel.webview.postMessage({ phase, text });
  }
}

function html(): string {
  const nonce = Math.random().toString(36).slice(2);
  return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';"><style>
body{margin:0;background:var(--vscode-editor-background);color:var(--vscode-editor-foreground);font:14px var(--vscode-font-family);display:grid;place-items:center;height:100vh}.wrap{text-align:center}.rings{width:180px;height:180px;position:relative;margin:auto}.ring{position:absolute;inset:0;border:2px solid var(--vscode-focusBorder);border-radius:50%;animation:spin 1.6s linear infinite}.ring:nth-child(2){inset:16px;border-color:var(--vscode-terminal-ansiMagenta)}.ring:nth-child(3){inset:34px;border-color:var(--vscode-terminal-ansiCyan);animation-direction:reverse}.ring.emit{border-color:#fff;background:radial-gradient(circle,rgba(255,255,255,.8),transparent 65%);animation:emit .7s ease-out infinite}@keyframes spin{to{transform:rotate(360deg)}}@keyframes emit{50%{transform:scale(1.18);opacity:.4}}h2{margin:18px 0 8px}p{color:var(--vscode-descriptionForeground)}.failure .ring{border-color:var(--vscode-errorForeground);animation:none}.success .ring{border-color:#62d39a;animation:emit .9s ease-in-out infinite}</style></head><body><main class="wrap"><div id="visual" class="rings"><i class="ring"></i><i class="ring"></i><i class="ring"></i></div><h2 id="phase">准备打包</h2><p id="text">正在检查项目构建命令…</p></main><script nonce="${nonce}">const labels={preparing:'准备打包',absorbing:'黑洞正在吸收源文件',building:'正在执行真实构建任务',emitting:'白洞正在生成产物',success:'打包完成',failure:'打包失败'};addEventListener('message',e=>{const d=e.data;document.getElementById('phase').textContent=labels[d.phase]||d.phase;document.getElementById('text').textContent=d.text||'';const v=document.getElementById('visual');v.className='rings '+(d.phase==='emitting'?'emit ': '')+(d.phase==='success'?'success ':'')+(d.phase==='failure'?'failure ':'')});</script></body></html>`;
}
