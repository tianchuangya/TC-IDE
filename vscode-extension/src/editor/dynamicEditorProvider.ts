/* eslint-disable no-useless-escape -- browser regexes are embedded as a template string. */
/**
 * Optional TC dynamic editor.
 *
 * Native VS Code editors do not expose their text DOM, so this provider uses
 * the official CustomTextEditor API instead of patching VS Code. The document
 * remains the source of truth: each edit is a WorkspaceEdit and therefore
 * participates in normal dirty state, undo/redo and save.
 */
import * as vscode from 'vscode';

export class DynamicEditorProvider implements vscode.CustomTextEditorProvider {
  static readonly viewType = 'tcIde.dynamicEditor';
  constructor(private readonly context: vscode.ExtensionContext) {}

  static register(context: vscode.ExtensionContext): vscode.Disposable {
    return vscode.window.registerCustomEditorProvider(DynamicEditorProvider.viewType, new DynamicEditorProvider(context), {
      webviewOptions: { retainContextWhenHidden: true },
      supportsMultipleEditorsPerDocument: true,
    });
  }

  async resolveCustomTextEditor(document: vscode.TextDocument, panel: vscode.WebviewPanel): Promise<void> {
    panel.webview.options = { enableScripts: true, localResourceRoots: [this.context.extensionUri] };
    panel.webview.html = this.html(document);
    let applying = false;
    const change = vscode.workspace.onDidChangeTextDocument((event) => {
      if (event.document.uri.toString() !== document.uri.toString() || applying) return;
      void panel.webview.postMessage({ type: 'replace', text: document.getText(), language: document.languageId });
    });
    panel.onDidDispose(() => change.dispose());
    panel.webview.onDidReceiveMessage(async (raw: unknown) => {
      if (!raw || typeof raw !== 'object') return;
      const message = raw as { type?: string; text?: string };
      if (message.type !== 'edit' || typeof message.text !== 'string') return;
      // A whole-document replacement is intentional here: the browser owns
      // the temporary visual buffer; VS Code remains authoritative and gives
      // this edit normal undo/redo and dirty tracking.
      applying = true;
      try {
        const edit = new vscode.WorkspaceEdit();
        const full = new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length));
        edit.replace(document.uri, full, message.text);
        await vscode.workspace.applyEdit(edit);
      } finally {
        applying = false;
      }
    });
  }

  private html(document: vscode.TextDocument): string {
    const nonce = Math.random().toString(36).slice(2);
    const source = JSON.stringify(document.getText()).replace(/</g, '\\u003c');
    const language = JSON.stringify(document.languageId);
    return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}'"><style>
:root{--line:24px;--pad:22px}html,body{margin:0;height:100%;overflow:hidden;background:var(--vscode-editor-background);color:var(--vscode-editor-foreground);font-family:var(--vscode-editor-font-family);font-size:var(--vscode-editor-font-size);line-height:var(--line)}#shell{position:relative;height:100%;overflow:auto;background:radial-gradient(circle at 82% 8%,rgba(70,145,255,.12),transparent 28%)}#code,#input{position:absolute;inset:0;margin:0;padding:var(--pad);border:0;box-sizing:border-box;tab-size:4;font:inherit;line-height:var(--line);white-space:pre;overflow:auto;min-height:100%;min-width:100%}#code{z-index:1;pointer-events:none;color:var(--vscode-editor-foreground)!important;-webkit-text-fill-color:var(--vscode-editor-foreground)!important}#input{z-index:2;resize:none;color:transparent!important;background:transparent!important;caret-color:var(--vscode-editorCursor-foreground);outline:none;-webkit-text-fill-color:transparent!important;selection-background:rgba(80,155,255,.35)}.glyph{position:absolute;z-index:5;pointer-events:none;color:#b8ddff;text-shadow:0 0 16px rgba(78,174,255,.95);font:inherit;animation:glyphIn .26s cubic-bezier(.17,.89,.32,1.25) both}@keyframes glyphIn{0%{opacity:0;transform:translateY(7px) scale(.72)}55%{opacity:1;transform:translateY(-2px) scale(1.18)}100%{opacity:0;transform:translateY(-9px) scale(.95)}}.erase{color:#ff8da1;animation:erase .22s ease-out both}@keyframes erase{from{opacity:1;transform:scale(1)}to{opacity:0;transform:translateX(-10px) scale(.4)}}#badge{position:fixed;z-index:4;right:14px;top:10px;color:var(--vscode-descriptionForeground);font:11px var(--vscode-font-family);padding:4px 8px;border:1px solid var(--vscode-widget-border);border-radius:10px;background:var(--vscode-editorWidget-background)}</style></head><body><div id="shell"><pre id="code" aria-hidden="true"></pre><textarea id="input" spellcheck="false" aria-label="TC 动态编辑器"></textarea><span id="badge">TC 动态编辑器 · ${escapeHtml(document.languageId)}</span></div><script nonce="${nonce}">const vscode=acquireVsCodeApi(),input=document.getElementById('input'),code=document.getElementById('code'),shell=document.getElementById('shell');let text=${source},language=${language},previous=text;input.value=text;function render(){code.textContent=text+'\n'}function caretPoint(){const before=input.value.slice(0,input.selectionStart),lines=before.split('\n'),line=lines.length-1,col=lines.at(-1).length;return{x:22+col*8.45-input.scrollLeft,y:22+line*24-input.scrollTop}}function glyph(ch,erase=false){if(!ch)return;const p=caretPoint(),g=document.createElement('span');g.className='glyph'+(erase?' erase':'');g.textContent=ch;g.style.left=p.x+'px';g.style.top=p.y+'px';shell.appendChild(g);setTimeout(()=>g.remove(),280)}function sync(){code.scrollTop=input.scrollTop;code.scrollLeft=input.scrollLeft}input.addEventListener('scroll',sync);input.addEventListener('input',()=>{text=input.value;const add=text.length>previous.length?text.slice(input.selectionStart-(text.length-previous.length),input.selectionStart):'';const remove=text.length<previous.length?previous.slice(input.selectionStart,input.selectionStart+(previous.length-text.length)):'';glyph(add||remove,!!remove);previous=text;render();sync();vscode.postMessage({type:'edit',text})});addEventListener('message',e=>{if(e.data.type==='replace'){text=e.data.text;previous=text;language=e.data.language;input.value=text;render();sync()}});render();sync();</script></body></html>`;
  }
}

function escapeHtml(value: string): string { return value.replace(/[&<>]/g, (item) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[item] ?? item); }
