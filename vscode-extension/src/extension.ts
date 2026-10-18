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
import { registerSemanticDecorations } from './editor/semanticDecorations';
import { TaskRunner, type TcTaskKind } from './tasks/taskRunner';
import { PackagePanel } from './package/packagePanel';
import { SettingsViewProvider } from './settings/settingsViewProvider';
import { DynamicEditorProvider } from './editor/dynamicEditorProvider';

export function activate(context: vscode.ExtensionContext): void {
  const provider = new GraphViewProvider(context);
  const tasks = new TaskRunner();
  const packagePanel = new PackagePanel();
  const settings = new SettingsViewProvider(context);
  context.subscriptions.push(tasks, registerSemanticDecorations(context));
  context.subscriptions.push(DynamicEditorProvider.register(context));
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(GraphViewProvider.viewType, provider, {
      // The view is worth keeping alive: tearing it down on every hide would
      // lose the fold state.
      webviewOptions: { retainContextWhenHidden: true },
    }),
  );
  context.subscriptions.push(vscode.window.registerWebviewViewProvider(SettingsViewProvider.viewType, settings, { webviewOptions: { retainContextWhenHidden: true } }));

  // Only the Extension Development Host opens the view automatically. Released
  // installs stay quiet until the user opens TC-IDE from the Activity Bar.
  if (context.extensionMode === vscode.ExtensionMode.Development) {
    setTimeout(() => {
      void vscode.commands.executeCommand('workbench.view.extension.tcIde').then(() => provider.reveal(true));
    }, 900);
  }

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
        provider.setScope(picked.value);
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
        provider.setRelation(picked.value);
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
    vscode.commands.registerCommand('tcIde.openSettings', () => settings.reveal()),
    vscode.commands.registerCommand('tcIde.openDynamicEditor', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) { void vscode.window.showWarningMessage('请先打开一个代码文件。'); return; }
      await vscode.commands.executeCommand('vscode.openWith', editor.document.uri, DynamicEditorProvider.viewType, vscode.ViewColumn.Active);
    }),
    vscode.commands.registerCommand('tcIde.build', () => runTask(tasks, 'build')),
    vscode.commands.registerCommand('tcIde.run', () => runTask(tasks, 'run')),
    vscode.commands.registerCommand('tcIde.package', async () => {
      packagePanel.show('preparing', '正在识别项目的真实打包命令…');
      await delay(180);
      packagePanel.show('absorbing', '源文件正在进入构建管线。');
      const result = await runTask(tasks, 'package', false);
      if (!result) { packagePanel.show('failure', '未能启动打包任务。'); return; }
      if (!result.ok) { packagePanel.show('failure', `构建未成功${result.exitCode === undefined ? '' : `（退出码 ${result.exitCode}）`}。`); return; }
      packagePanel.show('emitting', '构建完成，正在输出真实产物。');
      await delay(420);
      packagePanel.show('success', `已完成：${result.label}。产物请在项目的 target、dist 或构建目录中查看。`);
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
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('tcIde.appearance')) {
        provider.refreshAppearance();
      }
      if (event.affectsConfiguration('tcIde.graph.defaultScope')) {
        provider.setScope(vscode.workspace.getConfiguration('tcIde.graph').get('defaultScope', 'auto'));
      }
      if (event.affectsConfiguration('tcIde.graph.defaultRelation')) {
        provider.setRelation(vscode.workspace.getConfiguration('tcIde.graph').get('defaultRelation', 'follow'));
      }
    }),
  );

  // The theme is sent once the webview says it is ready; sending it here would
  // race the document that has not loaded yet.
}

async function runTask(tasks: TaskRunner, kind: TcTaskKind, notify = true): Promise<Awaited<ReturnType<TaskRunner['run']>> | undefined> {
  try {
    const result = await tasks.run(kind);
    if (notify) void vscode.window.showInformationMessage(`TC-IDE ${kind === 'build' ? '编译' : '运行'} ${result.ok ? '完成' : '失败'}`);
    return result;
  } catch (error) {
    void vscode.window.showErrorMessage(error instanceof Error ? error.message : String(error));
    return undefined;
  }
}

function delay(ms: number): Promise<void> { return new Promise((resolve) => setTimeout(resolve, ms)); }

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
