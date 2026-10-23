/** Lightweight editor semantics backed by VS Code language providers. */
import * as vscode from 'vscode';

export function registerSemanticDecorations(context: vscode.ExtensionContext): vscode.Disposable {
  const focus = vscode.window.createTextEditorDecorationType({
    border: '1px solid var(--vscode-focusBorder)', borderRadius: '3px',
    backgroundColor: 'rgba(0, 122, 204, .12)', overviewRulerColor: '#3b9cff', overviewRulerLane: vscode.OverviewRulerLane.Center,
  });
  const definition = vscode.window.createTextEditorDecorationType({
    border: '1px solid rgba(80, 210, 160, .82)', borderRadius: '3px', overviewRulerColor: '#50d2a0', overviewRulerLane: vscode.OverviewRulerLane.Left,
  });
  const reference = vscode.window.createTextEditorDecorationType({
    backgroundColor: 'rgba(183, 116, 255, .13)', borderRadius: '2px', overviewRulerColor: '#b774ff', overviewRulerLane: vscode.OverviewRulerLane.Right,
  });
  // VS Code decorations cannot run CSS transitions in the editor. These three
  // frames are deliberately distinct, then swapped on a short timer: this is
  // a visible fade-in → peak → fade-out rather than a one-frame outline.
  const pulseIn = vscode.window.createTextEditorDecorationType({ border: '1px solid rgba(96, 185, 255, .24)', borderRadius: '4px', backgroundColor: 'rgba(54, 155, 255, .05)' });
  const pulsePeak = vscode.window.createTextEditorDecorationType({ border: '2px solid rgba(96, 185, 255, 1)', borderRadius: '4px', backgroundColor: 'rgba(54, 155, 255, .30)' });
  const pulseOut = vscode.window.createTextEditorDecorationType({ border: '1px solid rgba(96, 185, 255, .20)', borderRadius: '4px', backgroundColor: 'rgba(54, 155, 255, .035)' });
  const glowIn = vscode.window.createTextEditorDecorationType({ border: '1px solid rgba(176, 115, 255, .22)', borderRadius: '4px', backgroundColor: 'rgba(166, 100, 255, .04)' });
  const glowPeak = vscode.window.createTextEditorDecorationType({ border: '2px solid rgba(176, 115, 255, .96)', borderRadius: '4px', backgroundColor: 'rgba(166, 100, 255, .28)', overviewRulerColor: '#A664FF', overviewRulerLane: vscode.OverviewRulerLane.Center });
  const glowOut = vscode.window.createTextEditorDecorationType({ border: '1px solid rgba(176, 115, 255, .18)', borderRadius: '4px', backgroundColor: 'rgba(166, 100, 255, .03)' });
  const deleted = vscode.window.createTextEditorDecorationType({ before: { contentText: '│', color: '#78BAFF', fontWeight: 'bold' } });
  const bubble = vscode.window.createTextEditorDecorationType({
    after: { margin: '0 0 0 14px', color: 'var(--vscode-descriptionForeground)', backgroundColor: 'var(--vscode-editorWidget-background)', border: '1px solid var(--vscode-widget-border)' },
  });
  let timer: ReturnType<typeof setTimeout> | undefined;
  const flashTimers: ReturnType<typeof setTimeout>[] = [];
  let sequence = 0;

  const clear = (): void => {
    for (const editor of vscode.window.visibleTextEditors) {
      editor.setDecorations(focus, []); editor.setDecorations(definition, []); editor.setDecorations(reference, []); clearTypingFrames(editor); editor.setDecorations(deleted, []); editor.setDecorations(bubble, []);
    }
  };
  const update = (): void => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { void refresh(); }, 260);
  };
  const refresh = async (): Promise<void> => {
    const editor = vscode.window.activeTextEditor;
    const enabled = vscode.workspace.getConfiguration('tcIde.editor').get('decorations', true);
    if (!editor || !enabled || editor.selection.isEmpty === false) { clear(); return; }
    const range = editor.document.getWordRangeAtPosition(editor.selection.active);
    if (!range) { clear(); return; }
    const current = ++sequence;
    editor.setDecorations(focus, [range]);
    renderCommentBubbles(editor, bubble);
    const max = vscode.workspace.getConfiguration('tcIde.editor').get<number>('maxDecorations', 12);
    try {
      const [definitions, highlights] = await Promise.all([
        vscode.commands.executeCommand<vscode.Location[]>('vscode.executeDefinitionProvider', editor.document.uri, editor.selection.active),
        vscode.commands.executeCommand<vscode.DocumentHighlight[]>('vscode.executeDocumentHighlights', editor.document.uri, editor.selection.active),
      ]);
      if (current !== sequence || vscode.window.activeTextEditor !== editor) return;
      const sameFile = (definitions ?? []).filter((item) => item.uri.toString() === editor.document.uri.toString()).slice(0, max);
      editor.setDecorations(definition, sameFile.map((item) => item.range));
      const references = (highlights ?? []).filter((item) => !item.range.isEqual(range)).slice(0, max);
      editor.setDecorations(reference, references.map((item) => item.range));
    } catch { /* language service unavailable: keep the local focus only. */ }
  };
  const flash = (event: vscode.TextDocumentChangeEvent): void => {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document !== event.document || event.contentChanges.length === 0) return;
    const inserted: vscode.Range[] = [];
    const removed: vscode.Range[] = [];
    for (const change of event.contentChanges.slice(0, 4)) {
      if (change.text.length > 0) {
        const startOffset = editor.document.offsetAt(change.range.start);
        inserted.push(new vscode.Range(change.range.start, editor.document.positionAt(startOffset + change.text.length)));
      } else {
        removed.push(new vscode.Range(change.range.start, change.range.start));
      }
    }
    const effect = vscode.workspace.getConfiguration('tcIde.editor').get<string>('typingEffect', 'pulse');
    if (effect === 'none') return;
    const frames: [vscode.TextEditorDecorationType, vscode.TextEditorDecorationType, vscode.TextEditorDecorationType] = effect === 'glow' ? [glowIn, glowPeak, glowOut] : [pulseIn, pulsePeak, pulseOut];
    clearTypingFrames(editor);
    editor.setDecorations(frames[0], inserted);
    editor.setDecorations(deleted, removed);
    flashTimers.push(setTimeout(() => { editor.setDecorations(frames[0], []); editor.setDecorations(frames[1], inserted); }, 42));
    flashTimers.push(setTimeout(() => { editor.setDecorations(frames[1], []); editor.setDecorations(frames[2], inserted); }, 108));
    flashTimers.push(setTimeout(() => { editor.setDecorations(frames[2], []); editor.setDecorations(deleted, []); }, 185));
  };
  const codeLens = vscode.languages.registerCodeLensProvider({ scheme: 'file' }, {
    async provideCodeLenses(document) {
      const symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>('vscode.executeDocumentSymbolProvider', document.uri) ?? [];
      return symbols.filter((symbol) => [vscode.SymbolKind.Function, vscode.SymbolKind.Method, vscode.SymbolKind.Class, vscode.SymbolKind.Struct].includes(symbol.kind)).slice(0, 30).map((symbol) => new vscode.CodeLens(symbol.selectionRange, {
        title: 'TC-IDE：在关系图中查看', command: 'tcIde.openGraph', tooltip: `打开 ${symbol.name} 的语义关系`,
      }));
    },
  });
  const clearTypingFrames = (editor: vscode.TextEditor): void => {
    while (flashTimers.length) clearTimeout(flashTimers.pop());
    for (const frame of [pulseIn, pulsePeak, pulseOut, glowIn, glowPeak, glowOut]) editor.setDecorations(frame, []);
  };
  const disposables: vscode.Disposable[] = [focus, definition, reference, pulseIn, pulsePeak, pulseOut, glowIn, glowPeak, glowOut, deleted, bubble, codeLens,
    vscode.window.onDidChangeTextEditorSelection(update), vscode.window.onDidChangeActiveTextEditor(update),
    vscode.workspace.onDidChangeTextDocument((event) => { flash(event); update(); }),
    vscode.workspace.onDidChangeConfiguration((event) => { if (event.affectsConfiguration('tcIde.editor')) update(); }),
  ];
  update();
  const disposable = new vscode.Disposable(() => { if (timer) clearTimeout(timer); while (flashTimers.length) clearTimeout(flashTimers.pop()); clear(); disposables.forEach((item) => item.dispose()); });
  context.subscriptions.push(disposable);
  return disposable;
}

function renderCommentBubbles(editor: vscode.TextEditor, decoration: vscode.TextEditorDecorationType): void {
  const config = vscode.workspace.getConfiguration('tcIde.editor');
  if (!config.get('commentBubbles', true)) { editor.setDecorations(decoration, []); return; }
  const style = config.get<string>('commentBubbleStyle', 'soft');
  const pattern = commentPattern(editor.document.languageId);
  if (!pattern) { editor.setDecorations(decoration, []); return; }
  const options: vscode.DecorationOptions[] = [];
  const visible = editor.visibleRanges[0];
  const firstLine = visible ? Math.max(0, visible.start.line - 20) : 0;
  const lastLine = visible ? Math.min(editor.document.lineCount, visible.end.line + 20) : Math.min(editor.document.lineCount, 120);
  for (let line = firstLine; line < lastLine && options.length < 12; line += 1) {
    const text = editor.document.lineAt(line).text;
    const match = pattern.exec(text); pattern.lastIndex = 0;
    if (!match || !match[1]) continue;
    const label = match[1].trim().replace(/\s+/g, ' ').slice(0, 42);
    if (!label) continue;
    options.push({ range: new vscode.Range(line, text.length, line, text.length), renderOptions: { after: { contentText: ` ${style === 'minimal' ? '注释' : label} ` } } });
  }
  editor.setDecorations(decoration, options);
}

function commentPattern(language: string): RegExp | undefined {
  if (['python', 'shellscript', 'yaml', 'toml', 'makefile'].includes(language)) return /#\s*(.*)$/;
  if (['c', 'cpp', 'java', 'javascript', 'typescript', 'rust', 'go', 'csharp'].includes(language)) return /\/\/\s*(.*)$/;
  return undefined;
}
