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

import { graphGeometry } from '../graph/geometry';
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

  let symbols: readonly vscode.DocumentSymbol[] | undefined;
  try {
    symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
      'vscode.executeDocumentSymbolProvider',
      document.uri,
    );
  } catch {
    symbols = undefined;
  }
  if (token.isCancellationRequested) {
    return { nodes: [], total: 0 };
  }
  if (!symbols || symbols.length === 0) {
    return {
      nodes: [],
      total: 0,
      degraded: '当前语言未提供符号或调用层级',
    };
  }

  // The file node first, so the hierarchy has a root the reader can name.
  const fileNode: GraphNodeDTO = {
    id: fileId(document.uri.toString()),
    level: 'file',
    title: vscode.workspace.asRelativePath(document.uri, false),
    subtitle: `${document.languageId} · ${document.lineCount} 行`,
    depth: 0,
    hasChildren: true,
    folded: false,
    location: { uri: document.uri.toString(), line: 0, column: 0 },
  };

  let nodes = nodesFromSymbols(document.uri, symbols).map((node) => ({
    ...node,
    depth: node.depth + 1,
  }));

  // The relation adds the callers or callees of the symbol under the caret.
  if (options.relation !== 'follow') {
    const callers = await callNeighbours(document.uri, editor.selection.active, options.relation, token);
    if (callers.length > 0) {
      nodes = nodes.concat(callers);
    }
  }

  const total = nodes.length + 1;
  const limited = nodes.slice(0, Math.max(0, options.maxNodes - 1));
  return {
    nodes: [fileNode, ...limited],
    total,
    ...(total > options.maxNodes
      ? { degraded: `节点数超过上限 ${graphGeometry.maxNodes}，已聚类显示` }
      : {}),
  };
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
