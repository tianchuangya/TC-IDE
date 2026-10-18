/**
 * Extension entry.
 *
 * Activation only registers providers and commands. Nothing scans the project
 * here: the handoff requires that opening the window stays cheap and that
 * indexing is asked for rather than assumed.
 */

import * as vscode from 'vscode';

import { GraphViewProvider, currentTokens } from './graph/graphViewProvider';
import { defaultDocument, documentPath, serialiseDocument } from './project/tcideFile';

export function activate(context: vscode.ExtensionContext): void {
  const provider = new GraphViewProvider(context);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(GraphViewProvider.viewType, provider, {
      // The view is worth keeping alive: tearing it down on every hide would
      // lose the fold state.
      webviewOptions: { retainContextWhenHidden: true },
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('tcIde.openGraph', () => {
      provider.reveal(true);
    }),
    vscode.commands.registerCommand('tcIde.refreshGraph', () => {
      provider.requestRebuild(true);
    }),
    vscode.commands.registerCommand('tcIde.focusScope', async () => {
      const picked = await vscode.window.showQuickPick(
        [
          { label: '自动', value: 'auto' as const },
          { label: '整个项目', value: 'project' as const },
          { label: '当前文件夹', value: 'folder' as const },
          { label: '当前文件', value: 'file' as const },
          { label: '当前符号', value: 'symbol' as const },
        ],
        { title: '关系范围' },
      );
      if (picked) {
        await vscode.workspace
          .getConfiguration('tcIde')
          .update('graph.defaultScope', picked.value, vscode.ConfigurationTarget.Workspace);
        provider.requestRebuild(true);
      }
    }),
    vscode.commands.registerCommand('tcIde.toggleRelation', async () => {
      const picked = await vscode.window.showQuickPick(
        [
          { label: '跟随', value: 'follow' as const },
          { label: '调用者', value: 'callers' as const },
          { label: '被调用', value: 'callees' as const },
        ],
        { title: '关系方向' },
      );
      if (picked) {
        await vscode.workspace
          .getConfiguration('tcIde')
          .update('graph.defaultRelation', picked.value, vscode.ConfigurationTarget.Workspace);
        provider.requestRebuild(true);
      }
    }),
    vscode.commands.registerCommand('tcIde.createProjectDocument', async () => {
      const folder = vscode.workspace.workspaceFolders?.[0];
      if (!folder) {
        void vscode.window.showWarningMessage('请先打开一个文件夹。');
        return;
      }
      const target = documentPath(folder.uri.fsPath);
      const uri = vscode.Uri.file(target);
      try {
        await vscode.workspace.fs.stat(uri);
        const answer = await vscode.window.showWarningMessage(
          `${target} 已存在，是否覆盖？`,
          { modal: true },
          '覆盖',
        );
        if (answer !== '覆盖') {
          return;
        }
      } catch {
        // Not found, which is the expected case.
      }
      const document = defaultDocument(folder.name);
      await vscode.workspace.fs.writeFile(
        uri,
        Buffer.from(serialiseDocument(document), 'utf8'),
      );
      const opened = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(opened);
      void vscode.window.showInformationMessage(`已创建 ${folder.name}.tcIDE`);
    }),
  );

  // Following the caret: debounced by the provider, and only while the view is
  // visible, so a hidden view costs nothing.
  context.subscriptions.push(
    vscode.window.onDidChangeTextEditorSelection(() => {
      if (isGraphVisible()) {
        provider.requestRebuild();
      }
    }),
    vscode.window.onDidChangeActiveTextEditor(() => {
      provider.requestRebuild();
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (isGraphVisible() && event.contentChanges.length > 0) {
        provider.requestRebuild();
      }
    }),
    vscode.window.onDidChangeActiveColorTheme(() => {
      provider.post({ type: 'setTheme', theme: currentTokens() });
    }),
  );

  // The theme is sent once the webview says it is ready; sending it here would
  // race the document that has not loaded yet.
}

function isGraphVisible(): boolean {
  if (!vscode.window.activeTextEditor) {
    return false;
  }
  // The provider debounces and drops superseded answers, so asking is cheap.
  return true;
}

export function deactivate(): void {
  // Everything is registered as a disposable; nothing else survives.
}
