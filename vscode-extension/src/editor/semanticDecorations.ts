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
  const changed = vscode.window.createTextEditorDecorationType({
    border: '1px solid rgba(68, 190, 255, .9)', borderRadius: '3px',
    backgroundColor: 'rgba(68, 190, 255, .18)',
  });
  let timer: ReturnType<typeof setTimeout> | undefined;
  let flashTimer: ReturnType<typeof setTimeout> | undefined;
  let sequence = 0;

  const clear = (): void => {
    for (const editor of vscode.window.visibleTextEditors) {
      editor.setDecorations(focus, []); editor.setDecorations(definition, []); editor.setDecorations(reference, []);
    }
  };
  const update = (): void => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { void refresh(); }, 90);
  };
  const refresh = async (): Promise<void> => {
    const editor = vscode.window.activeTextEditor;
    const enabled = vscode.workspace.getConfiguration('tcIde.editor').get('decorations', true);
    if (!editor || !enabled || editor.selection.isEmpty === false) { clear(); return; }
    const range = editor.document.getWordRangeAtPosition(editor.selection.active);
    if (!range) { clear(); return; }
    const current = ++sequence;
    editor.setDecorations(focus, [range]);
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
    const options = event.contentChanges.slice(0, 8).map((change) => {
      const first = change.text.split(/\r?\n/, 1)[0] ?? '';
      const end = new vscode.Position(change.range.start.line, change.range.start.character + Math.max(1, first.length));
      return new vscode.Range(change.range.start, end);
    });
    editor.setDecorations(changed, options);
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = setTimeout(() => editor.setDecorations(changed, []), 240);
  };
  const codeLens = vscode.languages.registerCodeLensProvider({ scheme: 'file' }, {
    async provideCodeLenses(document) {
      const symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>('vscode.executeDocumentSymbolProvider', document.uri) ?? [];
      return symbols.filter((symbol) => [vscode.SymbolKind.Function, vscode.SymbolKind.Method, vscode.SymbolKind.Class, vscode.SymbolKind.Struct].includes(symbol.kind)).slice(0, 30).map((symbol) => new vscode.CodeLens(symbol.selectionRange, {
        title: 'TC-IDE：在关系图中查看', command: 'tcIde.openGraph', tooltip: `打开 ${symbol.name} 的语义关系`,
      }));
    },
  });
  const disposables: vscode.Disposable[] = [focus, definition, reference, changed, codeLens,
    vscode.window.onDidChangeTextEditorSelection(update), vscode.window.onDidChangeActiveTextEditor(update),
    vscode.workspace.onDidChangeTextDocument((event) => { flash(event); update(); }),
    vscode.workspace.onDidChangeConfiguration((event) => { if (event.affectsConfiguration('tcIde.editor')) update(); }),
  ];
  update();
  const disposable = new vscode.Disposable(() => { if (timer) clearTimeout(timer); if (flashTimer) clearTimeout(flashTimer); clear(); disposables.forEach((item) => item.dispose()); });
  context.subscriptions.push(disposable);
  return disposable;
}
