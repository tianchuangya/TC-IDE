/**
 * The webview view that shows the graph.
 *
 * The provider owns the debouncing and cancellation: a cursor that moves
 * quickly must not queue one full analysis per keystroke, and an answer that
 * arrives after the focus moved on must not overwrite the newer one.
 */

import * as vscode from 'vscode';

import { contentSecurityPolicy, markup, styles } from './view';
import { parseGraphToHost, type GraphRelation, type GraphScope, type HostToGraph, type HostToGraphBody, type GraphToHost } from './protocol';
import type { ThemeTokens } from './protocol';
import { buildGraph } from '../semantic/indexer';
import { darkPalette, lightPalette, toCssVariables, levelColor } from '../theme';

/** How long the cursor must rest before a rebuild, in ms. */
const FOLLOW_DEBOUNCE_MS = 300;

export class GraphViewProvider implements vscode.WebviewViewProvider {
  public static readonly viewType = 'tcIde.graphView';

  private view: vscode.WebviewView | undefined;
  private scope: GraphScope = 'auto';
  private relation: GraphRelation = 'follow';
  private query = '';
  private readonly folded = new Set<string>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  private inFlight: vscode.CancellationTokenSource | undefined;
  /** Incremented per request so a stale answer can be ignored. */
  private requestSeq = 0;
  private lastRequestId = '';

  constructor(private readonly context: vscode.ExtensionContext) {}

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    view.webview.options = {
      enableScripts: true,
      localResourceRoots: [this.context.extensionUri],
    };
    view.webview.html = this.html(view.webview);
    view.webview.onDidReceiveMessage((raw: unknown) => {
      const message = parseGraphToHost(raw);
      if (!message) {
        // Anything unrecognised is dropped and logged by name only, never acted
        // on; the webview is a separate document and its messages are input.
        console.warn('[tc-ide] ignored an unrecognised webview message');
        return;
      }
      void this.onMessage(message);
    });
  }

  private html(webview: vscode.Webview): string {
    const nonce = makeNonce();
    const bundle = webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, 'dist', 'webview.js'));
    const csp = contentSecurityPolicy(bundle.toString(), nonce);
    return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8" />
<meta http-equiv="Content-Security-Policy" content="${csp}" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<style nonce="${nonce}">${styles({ nonce })}</style>
</head>
<body>
${markup()}
<script nonce="${nonce}" src="${bundle}"></script>
</body>
</html>`;
  }

  private async onMessage(message: GraphToHost): Promise<void> {
    switch (message.type) {
      case 'ready':
        this.reveal(true);
        break;
      case 'refresh':
        this.requestRebuild();
        break;
      case 'changeScope':
        this.scope = message.scope;
        this.requestRebuild();
        break;
      case 'changeRelation':
        this.relation = message.relation;
        this.requestRebuild();
        break;
      case 'search':
        this.query = message.query;
        this.requestRebuild();
        break;
      case 'fold':
        if (message.folded) {
          this.folded.add(message.nodeId);
        } else {
          this.folded.delete(message.nodeId);
        }
        break;
      case 'cancel':
        this.cancelInFlight();
        break;
      case 'openLocation': {
        try {
          const uri = vscode.Uri.parse(message.uri, true);
          const document = await vscode.workspace.openTextDocument(uri);
          const editor = await vscode.window.showTextDocument(document, { preview: true });
          const position = new vscode.Position(
            Math.min(message.line, Math.max(0, document.lineCount - 1)),
            message.column,
          );
          editor.selection = new vscode.Selection(position, position);
          editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
        } catch (error) {
          void vscode.window.showErrorMessage(
            `无法打开位置：${error instanceof Error ? error.message : String(error)}`,
          );
        }
        break;
      }
    }
  }

  /** Ask for a rebuild after the cursor has rested. */
  requestRebuild(immediate = false): void {
    if (this.timer !== undefined) {
      clearTimeout(this.timer);
      this.timer = undefined;
    }
    if (immediate) {
      void this.rebuild();
      return;
    }
    this.timer = setTimeout(() => {
      this.timer = undefined;
      void this.rebuild();
    }, FOLLOW_DEBOUNCE_MS);
  }

  private cancelInFlight(): void {
    this.inFlight?.cancel();
    this.inFlight?.dispose();
    this.inFlight = undefined;
  }

  private async rebuild(): Promise<void> {
    if (!this.view) {
      return;
    }
    // A new request supersedes the old one, so an answer that arrives late is
    // dropped instead of drawing a graph for a cursor that has moved on.
    this.cancelInFlight();
    const source = new vscode.CancellationTokenSource();
    this.inFlight = source;
    this.requestSeq += 1;
    const requestId = `req-${this.requestSeq}`;
    this.lastRequestId = requestId;

    this.post({ type: 'setState', state: 'loading' });
    try {
      const result = await buildGraph(
        {
          scope: this.scope,
          relation: this.relation,
          maxNodes: this.maxNodes(),
        },
        source.token,
      );
      if (source.token.isCancellationRequested || this.lastRequestId !== requestId) {
        return;
      }
      this.post({
        type: 'replaceGraph',
        requestId,
        graph: {
          nodes: result.nodes.map((node) => ({
            ...node,
            folded: this.folded.has(node.id),
          })),
          total: result.total,
          scope: this.scope,
          relation: this.relation,
          ...(result.degraded ? { degraded: result.degraded } : {}),
        },
      });
    } catch (error) {
      if (source.token.isCancellationRequested) {
        return;
      }
      this.post({
        type: 'setState',
        state: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      if (this.inFlight === source) {
        this.inFlight = undefined;
      }
      source.dispose();
    }
  }

  private maxNodes(): number {
    const configured = vscode.workspace.getConfiguration('tcIde').get<number>('graph.maxNodes', 240);
    return Number.isFinite(configured) ? Math.min(Math.max(240 > configured ? configured : configured, 20), 2000) : 240;
  }

  /** Reveal the view and rebuild it. */
  reveal(immediate = false): void {
    void vscode.commands.executeCommand('tcIde.graphView.focus');
    this.post({ type: 'setTheme', theme: currentTokens() });
    this.requestRebuild(immediate);
  }

  post(message: HostToGraphBody): void {
    const withVersion: HostToGraph = { version: 1, ...message } as HostToGraph;
    void this.view?.webview.postMessage(withVersion);
  }

  dispose(): void {
    this.cancelInFlight();
    if (this.timer !== undefined) {
      clearTimeout(this.timer);
    }
  }
}

/** The theme tokens matching the editor's current colour theme. */
export function currentTokens(): ThemeTokens {
  const kind = vscode.window.activeColorTheme.kind;
  const light = kind === vscode.ColorThemeKind.Light || kind === vscode.ColorThemeKind.HighContrastLight;
  return light ? lightPalette : darkPalette;
}

function makeNonce(): string {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let text = '';
  for (let index = 0; index < 32; index += 1) {
    text += alphabet.charAt(Math.floor(Math.random() * alphabet.length));
  }
  return text;
}

export { toCssVariables, levelColor };
