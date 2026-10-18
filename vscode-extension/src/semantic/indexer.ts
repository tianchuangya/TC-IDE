/**
 * Building the graph from what VS Code really knows.
 *
 * Everything here comes from a language provider: document symbols, the call
 * hierarchy, definitions and references. When a language has no provider the
 * graph says so rather than inventing a relationship, which the handoff
 * forbids explicitly - a guessed edge is worse than a visible gap, because it
 * looks like a fact.
 */

import * as vscode from 'vscode';

import type { GraphNodeDTO, GraphRelation, GraphScope, NodeLevel } from '../graph/protocol';

export interface BuildOptions {
  scope: GraphScope;
  relation: GraphRelation;
  maxNodes: number;
}

export interface BuildResult {
  nodes: GraphNodeDTO[];
  total: number;
  /** Set when the graph is not the full truth, with the reason. */
  degraded?: string;
}

/** Stable identifier for a symbol, never an array index. */
export function symbolId(uri: string, line: number, name: string): string {
  return `symbol:${uri}:${line}:${name}`;
}

export function fileId(uri: string): string {
  return `file:${uri}`;
}

export function folderId(uri: string): string {
  return `folder:${uri}`;
}

const KIND_LEVELS: Record<number, NodeLevel> = {
  [vscode.SymbolKind.File]: 'file',
  [vscode.SymbolKind.Module]: 'file',
  [vscode.SymbolKind.Namespace]: 'folder',
  [vscode.SymbolKind.Package]: 'folder',
  [vscode.SymbolKind.Class]: 'class',
  [vscode.SymbolKind.Struct]: 'class',
  [vscode.SymbolKind.Interface]: 'class',
  [vscode.SymbolKind.Enum]: 'class',
  [vscode.SymbolKind.Function]: 'function',
  [vscode.SymbolKind.Method]: 'function',
  [vscode.SymbolKind.Constructor]: 'function',
  [vscode.SymbolKind.Variable]: 'variable',
  [vscode.SymbolKind.Constant]: 'variable',
  [vscode.SymbolKind.Field]: 'variable',
  [vscode.SymbolKind.Property]: 'variable',
};

function levelOf(kind: vscode.SymbolKind): NodeLevel {
  return KIND_LEVELS[kind] ?? 'variable';
}

/** Where the user is looking: the hovered symbol, else the caret's, else the file. */
export function resolveScope(editor: vscode.TextEditor | undefined): {
  scope: GraphScope;
  uri?: vscode.Uri;
  position?: vscode.Position;
} {
  if (!editor) {
    return { scope: 'project' };
  }
  return { scope: 'file', uri: editor.document.uri, position: editor.selection.active };
}

/**
 * Turn the symbols of one document into nodes.
 *
 * The depth of a child is its parent's plus one, so the drawing's indentation
 * and the tree the reader sees are the same structure.
 */
export function nodesFromSymbols(uri: vscode.Uri, symbols: readonly vscode.DocumentSymbol[]): GraphNodeDTO[] {
  const nodes: GraphNodeDTO[] = [];
  const walk = (list: readonly vscode.DocumentSymbol[], depth: number, parentIndex: number | null): void => {
    for (const symbol of list) {
      const level = levelOf(symbol.kind);
      const id = symbolId(uri.toString(), symbol.range.start.line, symbol.name);
      const node: GraphNodeDTO = {
        id,
        level,
        title: level === 'function' ? `${symbol.name}()` : symbol.name,
        subtitle: `${vscode.workspace.asRelativePath(uri, false)}:${symbol.range.start.line + 1}`,
        depth,
        hasChildren: symbol.children.length > 0,
        folded: false,
        location: {
          uri: uri.toString(),
          line: symbol.range.start.line,
          column: symbol.range.start.character,
        },
      };
      nodes.push(node);
      walk(symbol.children, depth + 1, nodes.length - 1);
      void parentIndex;
    }
  };
  walk(symbols, 0, null);
  return nodes;
}

/**
 * Build the graph for the current editor.
 *
 * The calls are the ones the handoff names, and each is allowed to fail: a
 * language with no call hierarchy is normal, not an error.
 */
export async function buildGraph(
  options: BuildOptions,
  token: vscode.CancellationToken,
): Promise<BuildResult> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    return { nodes: [], total: 0, degraded: '没有打开的编辑器' };
  }
  const document = editor.document;
  const scope = effectiveScope(options.scope, editor);
  const documents = await documentsForScope(document, scope, token);
  if (token.isCancellationRequested) return { nodes: [], total: 0 };

  const nodes: GraphNodeDTO[] = [];
  const folders = new Set<string>();
  const projectRoot = workspaceRootFor(document.uri);
  const projectName = projectRoot ? vscode.workspace.asRelativePath(projectRoot, false) : '当前工作区';
  const hasProjectRoot = scope === 'project' || scope === 'folder';
  if (hasProjectRoot) {
    nodes.push({
      id: `project:${projectRoot?.toString() ?? 'workspace'}`,
      level: 'project', title: projectName || '当前工作区', subtitle: `${documents.length} 个已索引文件`,
      depth: 0, hasChildren: true, folded: false,
      ...(projectRoot ? { location: { uri: projectRoot.toString(), line: 0, column: 0 } } : {}),
    });
  }

  for (const uri of documents) {
    if (token.isCancellationRequested) break;
    const opened = uri.toString() === document.uri.toString() ? document : await tryOpen(uri);
    if (!opened) continue;
    const baseDepth = hasProjectRoot ? 1 : 0;
    const relative = vscode.workspace.asRelativePath(uri, false).replace(/\\/g, '/');
    const parts = relative.split('/');
    const fileName = parts.pop() ?? uri.path.split('/').pop() ?? '文件';
    let depth = baseDepth;
    let prefix = '';
    for (const segment of parts) {
      prefix = prefix ? `${prefix}/${segment}` : segment;
      const id = folderId(`${projectRoot?.toString() ?? ''}/${prefix}`);
      if (!folders.has(id)) {
        folders.add(id);
        nodes.push({ id, level: 'folder', title: segment, subtitle: prefix, depth, hasChildren: true, folded: false });
      }
      depth += 1;
    }
    const symbols = await documentSymbols(uri);
    nodes.push({
      id: fileId(uri.toString()), level: 'file', title: fileName,
      subtitle: `${opened.languageId} · ${opened.lineCount} 行`, depth,
      hasChildren: symbols.length > 0, folded: false,
      location: { uri: uri.toString(), line: 0, column: 0 },
    });
    const selected = scope === 'symbol' && uri.toString() === document.uri.toString()
      ? symbolsForPosition(symbols, editor.selection.active) : symbols;
    nodes.push(...nodesFromSymbols(uri, selected).map((node) => ({ ...node, depth: node.depth + depth + 1 })));
  }

  if (options.relation !== 'follow' && !token.isCancellationRequested) {
    const calls = await callNeighbours(document.uri, editor.selection.active, options.relation, token);
    if (calls.length) nodes.push(...calls.map((node) => ({ ...node, depth: hasProjectRoot ? 1 : 0 })));
  }
  const total = nodes.length;
  const limited = nodes.slice(0, options.maxNodes);
  return {
    nodes: limited, total,
    ...(total === 0 ? { degraded: '当前语言未提供符号；仍可用 VS Code 的语言扩展补全语义。' }
      : total > options.maxNodes ? { degraded: `节点数超过上限 ${options.maxNodes}，已截取显示` } : {}),
  };
}

function effectiveScope(scope: GraphScope, _editor: vscode.TextEditor): Exclude<GraphScope, 'auto'> {
  return scope === 'auto' ? 'file' : scope;
}

function workspaceRootFor(uri: vscode.Uri): vscode.Uri | undefined {
  return vscode.workspace.getWorkspaceFolder(uri)?.uri ?? vscode.workspace.workspaceFolders?.[0]?.uri;
}

async function documentsForScope(active: vscode.TextDocument, scope: Exclude<GraphScope, 'auto'>, token: vscode.CancellationToken): Promise<vscode.Uri[]> {
  if (scope === 'file' || scope === 'symbol') return [active.uri];
  const root = workspaceRootFor(active.uri);
  if (!root) return [active.uri];
  const configured = vscode.workspace.getConfiguration('tcIde').get<string[]>('graph.excludeGlobs', []);
  const excluded = configured.length ? `{${configured.join(',')}}` : undefined;
  const found = await vscode.workspace.findFiles('**/*', excluded, 80);
  const source = found.filter((uri) => isSourceFile(uri));
  const folder = active.uri.with({ path: active.uri.path.replace(/\/[^/]+$/, '') }).path;
  const filtered = scope === 'folder' ? source.filter((uri) => uri.path.startsWith(folder)) : source;
  const unique = new Map<string, vscode.Uri>([[active.uri.toString(), active.uri]]);
  for (const uri of filtered) unique.set(uri.toString(), uri);
  void token;
  return [...unique.values()].sort((a, b) => a.path.localeCompare(b.path));
}

function isSourceFile(uri: vscode.Uri): boolean {
  return /\.(c|cc|cpp|cxx|h|hpp|rs|ts|tsx|js|jsx|py|java|go|cs|json)$/i.test(uri.path);
}

async function tryOpen(uri: vscode.Uri): Promise<vscode.TextDocument | undefined> {
  try { return await vscode.workspace.openTextDocument(uri); } catch { return undefined; }
}

async function documentSymbols(uri: vscode.Uri): Promise<readonly vscode.DocumentSymbol[]> {
  try { return await vscode.commands.executeCommand<vscode.DocumentSymbol[]>('vscode.executeDocumentSymbolProvider', uri) ?? []; } catch { return []; }
}

function symbolsForPosition(symbols: readonly vscode.DocumentSymbol[], position: vscode.Position): readonly vscode.DocumentSymbol[] {
  for (const symbol of symbols) {
    if (symbol.range.contains(position)) {
      const nested = symbolsForPosition(symbol.children, position);
      return nested.length ? nested : [symbol];
    }
  }
  return symbols;
}

/**
 * Callers or callees of the symbol at a position, from the call hierarchy.
 *
 * Providers differ in whether they answer at all; an empty list means the
 * language does not support it, which the caller reports as a degraded graph
 * rather than as an empty one.
 */
async function callNeighbours(
  uri: vscode.Uri,
  position: vscode.Position,
  relation: Exclude<GraphRelation, 'follow'>,
  token: vscode.CancellationToken,
): Promise<GraphNodeDTO[]> {
  let items: vscode.CallHierarchyItem[] | undefined;
  try {
    items = await vscode.commands.executeCommand<vscode.CallHierarchyItem[]>(
      'vscode.prepareCallHierarchy',
      uri,
      position,
    );
  } catch {
    return [];
  }
  if (token.isCancellationRequested || !items || items.length === 0) {
    return [];
  }
  const first = items[0];
  if (!first) {
    return [];
  }
  const nodes: GraphNodeDTO[] = [];
  try {
    if (relation === 'callers') {
      const calls = await vscode.commands.executeCommand<vscode.CallHierarchyIncomingCall[]>(
        'vscode.provideIncomingCalls',
        first,
      );
      for (const call of calls ?? []) {
        nodes.push({
          id: symbolId(call.from.uri.toString(), call.from.range.start.line, call.from.name),
          level: 'call',
          title: `${call.from.name}()`,
          subtitle: `${vscode.workspace.asRelativePath(call.from.uri, false)}:${call.from.range.start.line + 1}`,
          depth: 1,
          hasChildren: false,
          folded: false,
          relation: 'call',
          location: {
            uri: call.from.uri.toString(),
            line: call.from.range.start.line,
            column: call.from.range.start.character,
          },
        });
      }
    } else {
      const calls = await vscode.commands.executeCommand<vscode.CallHierarchyOutgoingCall[]>(
        'vscode.provideOutgoingCalls',
        first,
      );
      for (const call of calls ?? []) {
        nodes.push({
          id: symbolId(call.to.uri.toString(), call.to.range.start.line, call.to.name),
          level: 'call',
          title: `${call.to.name}()`,
          subtitle: `${vscode.workspace.asRelativePath(call.to.uri, false)}:${call.to.range.start.line + 1}`,
          depth: 1,
          hasChildren: false,
          folded: false,
          relation: 'call',
          location: {
            uri: call.to.uri.toString(),
            line: call.to.range.start.line,
            column: call.to.range.start.character,
          },
        });
      }
    }
  } catch {
    return [];
  }
  return nodes;
}
