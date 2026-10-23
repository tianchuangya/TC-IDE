import * as vscode from 'vscode';

const loaderId = 'be5invis.vscode-custom-css';
const cssSetting = 'vscode_custom_css.imports';

export function registerEnhancedMotion(context: vscode.ExtensionContext): vscode.Disposable {
  const enable = vscode.commands.registerCommand('tcIde.enableEnhancedMotion', async () => {
    if (!vscode.extensions.getExtension(loaderId)) {
      const choice = await vscode.window.showWarningMessage(
        'TC 增强动效需要 Custom CSS and JS Loader。它会修改 VS Code 工作台文件，更新 VS Code 后可能需要重新启用。',
        { modal: true },
        '打开扩展页面',
      );
      if (choice === '打开扩展页面') {
        await vscode.commands.executeCommand('extension.open', loaderId);
      }
      return;
    }

    const choice = await vscode.window.showWarningMessage(
      '即将让 Loader 把 TC 动效注入 VS Code。启用后需要重新加载窗口，VS Code 可能显示校验警告。',
      { modal: true },
      '确认启用',
    );
    if (choice !== '确认启用') return;
    const resources = resourceUrls(context);
    const current = vscode.workspace.getConfiguration().get<string[]>(cssSetting, []);
    const kept = current.filter((item) => !isTcMotionResource(item));
    await vscode.workspace.getConfiguration().update(cssSetting, [...kept, ...resources], vscode.ConfigurationTarget.Global);
    await vscode.commands.executeCommand('extension.updateCustomCSS');
  });

  const disable = vscode.commands.registerCommand('tcIde.disableEnhancedMotion', async () => {
    const current = vscode.workspace.getConfiguration().get<string[]>(cssSetting, []);
    await vscode.workspace.getConfiguration().update(cssSetting, current.filter((item) => !isTcMotionResource(item)), vscode.ConfigurationTarget.Global);
    if (vscode.extensions.getExtension(loaderId)) await vscode.commands.executeCommand('extension.updateCustomCSS');
  });
  return vscode.Disposable.from(enable, disable);
}

function resourceUrls(context: vscode.ExtensionContext): string[] {
  return ['tc-motion.css', 'tc-motion.js'].map((file) => vscode.Uri.file(context.asAbsolutePath(`media/injected/${file}`)).toString());
}

function isTcMotionResource(value: string): boolean {
  return /\/media\/injected\/tc-motion\.(?:css|js)(?:$|\?)/.test(value.replace(/\\/g, '/'));
}
