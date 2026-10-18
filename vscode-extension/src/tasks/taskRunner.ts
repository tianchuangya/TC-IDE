/** Real VS Code task integration. We never simulate a compiler or a terminal. */
import * as vscode from 'vscode';

export type TcTaskKind = 'build' | 'run' | 'package';
export interface TaskOutcome { ok: boolean; exitCode?: number; label: string; root: vscode.Uri; }
interface DetectedCommand { command: string; args: string[]; label: string; }

export class TaskRunner implements vscode.Disposable {
  private readonly output = vscode.window.createOutputChannel('TC-IDE Tasks');

  async run(kind: TcTaskKind): Promise<TaskOutcome> {
    const root = vscode.workspace.workspaceFolders?.[0]?.uri;
    if (!root) throw new Error('请先打开一个项目文件夹。');
    if (!vscode.workspace.isTrusted) throw new Error('工作区未被信任，已拒绝执行构建命令。');
    const detected = await detect(root, kind);
    if (!detected) throw new Error(`无法识别此项目的${labelFor(kind)}命令。请添加 Cargo.toml、package.json（对应 scripts）或 Makefile。`);
    const definition: vscode.TaskDefinition = { type: 'tc-ide', task: kind };
    const task = new vscode.Task(definition, vscode.TaskScope.Workspace, detected.label, 'TC-IDE', new vscode.ShellExecution(detected.command, detected.args));
    task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Shared, clear: false, focus: false };
    this.output.appendLine(`[${new Date().toLocaleTimeString()}] ${detected.command} ${detected.args.join(' ')}`);
    const execution = await vscode.tasks.executeTask(task);
    return await new Promise<TaskOutcome>((resolve) => {
      const processEnd = vscode.tasks.onDidEndTaskProcess((event) => {
        if (event.execution !== execution) return;
        processEnd.dispose(); taskEnd.dispose();
        const outcome = { ok: event.exitCode === 0, exitCode: event.exitCode, label: detected.label, root };
        this.output.appendLine(`${detected.label} ${outcome.ok ? '完成' : `失败（退出码 ${event.exitCode ?? '未知'}）`}`);
        resolve(outcome);
      });
      const taskEnd = vscode.tasks.onDidEndTask((event) => {
        if (event.execution !== execution) return;
        // Shell tasks normally emit onDidEndTaskProcess. If a task provider does
        // not, report it as not verified rather than inventing a successful build.
        setTimeout(() => { processEnd.dispose(); taskEnd.dispose(); resolve({ ok: false, label: detected.label, root }); }, 80);
      });
    });
  }

  show(): void { this.output.show(true); }
  dispose(): void { this.output.dispose(); }
}

async function detect(root: vscode.Uri, kind: TcTaskKind): Promise<DetectedCommand | undefined> {
  if (await exists(vscode.Uri.joinPath(root, 'Cargo.toml'))) {
    if (kind === 'build') return command('cargo', ['build'], 'Cargo Build');
    if (kind === 'run') return command('cargo', ['run'], 'Cargo Run');
    return command('cargo', ['build', '--release'], 'Cargo Release Build');
  }
  const packageJson = vscode.Uri.joinPath(root, 'package.json');
  if (await exists(packageJson)) {
    const scripts = await npmScripts(packageJson);
    const script = kind === 'build' ? 'build' : kind === 'run' ? (scripts.has('start') ? 'start' : 'dev') : 'package';
    if (scripts.has(script)) return command('npm', ['run', script], `npm run ${script}`);
  }
  if (await exists(vscode.Uri.joinPath(root, 'Makefile'))) {
    return command('make', kind === 'build' ? [] : [kind], kind === 'build' ? 'make' : `make ${kind}`);
  }
  return undefined;
}

function command(command: string, args: string[], label: string): DetectedCommand { return { command, args, label }; }
async function exists(uri: vscode.Uri): Promise<boolean> { try { await vscode.workspace.fs.stat(uri); return true; } catch { return false; } }
async function npmScripts(uri: vscode.Uri): Promise<Set<string>> {
  try { const data = JSON.parse(Buffer.from(await vscode.workspace.fs.readFile(uri)).toString('utf8')) as { scripts?: Record<string, unknown> }; return new Set(Object.keys(data.scripts ?? {}).filter((name) => typeof data.scripts?.[name] === 'string')); } catch { return new Set(); }
}
function labelFor(kind: TcTaskKind): string { return ({ build: '编译', run: '运行', package: '打包' })[kind]; }
