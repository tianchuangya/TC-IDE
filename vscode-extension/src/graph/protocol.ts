/**
 * The message protocol between the Extension Host and the graph webview.
 *
 * Both directions carry a version and a type, and every field is validated on
 * arrival: the webview is a separate document, so anything it sends is input
 * from outside the extension and must be treated as such.
 */

export const PROTOCOL_VERSION = 1 as const;

export type GraphScope = 'auto' | 'project' | 'folder' | 'file' | 'symbol';
export type GraphRelation = 'follow' | 'callers' | 'callees';
export type NodeLevel = 'project' | 'folder' | 'file' | 'class' | 'function' | 'variable' | 'call' | 'definition';
export type RelationKind = 'definition' | 'call' | 'reference';

export const SCOPES: readonly GraphScope[] = ['auto', 'project', 'folder', 'file', 'symbol'];
export const RELATIONS: readonly GraphRelation[] = ['follow', 'callers', 'callees'];

export interface GraphNodeDTO {
  /** Stable identity, never an array index. */
  id: string;
  level: NodeLevel;
  title: string;
  subtitle: string;
  depth: number;
  /** Whether this node has children that can be folded. */
  hasChildren: boolean;
  folded: boolean;
  /** Jump target, when the node stands for a real location. */
  location?: { uri: string; line: number; column: number };
  relation?: RelationKind;
}

export interface GraphSnapshot {
  nodes: GraphNodeDTO[];
  /** Total nodes before folding, so the footer can say what is hidden. */
  total: number;
  scope: GraphScope;
  relation: GraphRelation;
  /** Why the graph is what it is, when it is not a plain result. */
  degraded?: string;
}

export interface ThemeTokens {
  background: string;
  panel: string;
  raised: string;
  border: string;
  text: string;
  muted: string;
  bright: string;
  accent: string;
  selection: string;
  selectedText: string;
  /** Per-level colours, used for the marker beside a card. */
  level: Record<NodeLevel, string>;
  error: string;
}

export interface PackageProgress {
  phase: 'idle' | 'preparing' | 'absorbing' | 'building' | 'packaging' | 'emitting' | 'success' | 'failure' | 'cancelled';
  /** 0..1 within the current animated phase. */
  progress: number;
  /** Real task label, shown while building. */
  taskLabel?: string;
  exitCode?: number;
  artifacts?: { name: string; size: number; format: string }[];
  message?: string;
}

export type HostToGraph =
  | { version: 1; type: 'ready' }
  | { version: 1; type: 'replaceGraph'; requestId: string; graph: GraphSnapshot }
  | { version: 1; type: 'setState'; state: 'loading' | 'empty' | 'error' | 'cancelled'; message?: string }
  | { version: 1; type: 'focusNode'; nodeId: string }
  | { version: 1; type: 'setTheme'; theme: ThemeTokens }
  | { version: 1; type: 'setReducedMotion'; reduced: boolean }
  | { version: 1; type: 'packageProgress'; progress: PackageProgress };

/**
 * The same messages without their version, so the host can build one and have
 * the version attached in a single place instead of at every call site.
 */
export type HostToGraphBody =
  | { type: 'ready' }
  | { type: 'replaceGraph'; requestId: string; graph: GraphSnapshot }
  | { type: 'setState'; state: 'loading' | 'empty' | 'error' | 'cancelled'; message?: string }
  | { type: 'focusNode'; nodeId: string }
  | { type: 'setTheme'; theme: ThemeTokens }
  | { type: 'setReducedMotion'; reduced: boolean }
  | { type: 'packageProgress'; progress: PackageProgress };

export type GraphToHost =
  | { version: 1; type: 'ready' }
  | { version: 1; type: 'openLocation'; uri: string; line: number; column: number }
  | { version: 1; type: 'changeScope'; scope: GraphScope }
  | { version: 1; type: 'changeRelation'; relation: GraphRelation }
  | { version: 1; type: 'search'; query: string }
  | { version: 1; type: 'fold'; nodeId: string; folded: boolean }
  | { version: 1; type: 'cancel'; requestId: string }
  | { version: 1; type: 'refresh' };

const NODE_LEVELS: readonly NodeLevel[] = ['project', 'folder', 'file', 'class', 'function', 'variable', 'call', 'definition'];
const RELATION_KINDS: readonly RelationKind[] = ['definition', 'call', 'reference'];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

/**
 * Parse a message from the webview.
 *
 * Anything unrecognised returns `null` rather than throwing: a malformed
 * message must not take down the handler, and the caller logs and ignores it.
 */
export function parseGraphToHost(raw: unknown): GraphToHost | null {
  // The version must be a number, not a numeric string: `Number('1') === 1`
  // would let a differently shaped message through.
  if (!isRecord(raw) || raw['version'] !== PROTOCOL_VERSION || typeof raw['version'] !== 'number') {
    return null;
  }
  const type = raw['type'];
  switch (type) {
    case 'ready':
      return { version: 1, type: 'ready' };
    case 'openLocation': {
      const uri = raw['uri'];
      const line = raw['line'];
      const column = raw['column'];
      if (!isString(uri) || !isFiniteNumber(line) || !isFiniteNumber(column)) {
        return null;
      }
      // A jump target has to be a real scheme; anything else is refused here
      // rather than opened later.
      if (!isAllowedUri(uri)) {
        return null;
      }
      return {
        version: 1,
        type: 'openLocation',
        uri,
        line: Math.max(0, Math.floor(line)),
        column: Math.max(0, Math.floor(column)),
      };
    }
    case 'changeScope': {
      const scope = raw['scope'];
      if (!isString(scope) || !SCOPES.includes(scope as GraphScope)) {
        return null;
      }
      return { version: 1, type: 'changeScope', scope: scope as GraphScope };
    }
    case 'changeRelation': {
      const relation = raw['relation'];
      if (!isString(relation) || !RELATIONS.includes(relation as GraphRelation)) {
        return null;
      }
      return { version: 1, type: 'changeRelation', relation: relation as GraphRelation };
    }
    case 'search': {
      const query = raw['query'];
      if (!isString(query)) {
        return null;
      }
      return { version: 1, type: 'search', query: query.slice(0, 200) };
    }
    case 'fold': {
      const nodeId = raw['nodeId'];
      const folded = raw['folded'];
      if (!isString(nodeId) || typeof folded !== 'boolean') {
        return null;
      }
      return { version: 1, type: 'fold', nodeId, folded };
    }
    case 'cancel': {
      const requestId = raw['requestId'];
      if (!isString(requestId)) {
        return null;
      }
      return { version: 1, type: 'cancel', requestId };
    }
    case 'refresh':
      return { version: 1, type: 'refresh' };
    default:
      return null;
  }
}

/** Schemes a node may point at. */
const ALLOWED_SCHEMES = ['file:', 'untitled:', 'vscode-remote:', 'vscode-notebook-cell:'];

/** Whether a uri is one the extension will act on. */
export function isAllowedUri(uri: string): boolean {
  if (uri.length === 0 || uri.length > 4096) {
    return false;
  }
  const lower = uri.toLowerCase();
  return ALLOWED_SCHEMES.some((scheme) => lower.startsWith(scheme));
}

/**
 * Validate a node coming from the extension, so the webview can draw without
 * checking every field again at every use.
 */
export function parseNode(raw: unknown): GraphNodeDTO | null {
  if (!isRecord(raw)) {
    return null;
  }
  const id = raw['id'];
  const level = raw['level'];
  const title = raw['title'];
  const subtitle = raw['subtitle'];
  const depth = raw['depth'];
  if (!isString(id) || id.length === 0 || id.length > 512) {
    return null;
  }
  if (!isString(level) || !NODE_LEVELS.includes(level as NodeLevel)) {
    return null;
  }
  if (!isString(title) || !isString(subtitle) || !isFiniteNumber(depth)) {
    return null;
  }
  const node: GraphNodeDTO = {
    id,
    level: level as NodeLevel,
    title,
    subtitle,
    depth: Math.max(0, Math.floor(depth)),
    hasChildren: raw['hasChildren'] === true,
    folded: raw['folded'] === true,
  };
  const location = raw['location'];
  if (isRecord(location)) {
    const uri = location['uri'];
    if (isString(uri) && isAllowedUri(uri) && isFiniteNumber(location['line']) && isFiniteNumber(location['column'])) {
      node.location = {
        uri,
        line: Math.max(0, Math.floor(location['line'])),
        column: Math.max(0, Math.floor(location['column'])),
      };
    }
  }
  const relation = raw['relation'];
  if (isString(relation) && RELATION_KINDS.includes(relation as RelationKind)) {
    node.relation = relation as RelationKind;
  }
  return node;
}

/** Parse a whole snapshot, dropping nodes that fail validation. */
export function parseSnapshot(raw: unknown): GraphSnapshot | null {
  if (!isRecord(raw)) {
    return null;
  }
  const rawNodes = raw['nodes'];
  if (!Array.isArray(rawNodes)) {
    return null;
  }
  const nodes = rawNodes.map(parseNode).filter((node): node is GraphNodeDTO => node !== null);
  const scope = raw['scope'];
  const relation = raw['relation'];
  const total = raw['total'];
  return {
    nodes,
    total: isFiniteNumber(total) ? Math.max(0, Math.floor(total)) : nodes.length,
    scope: isString(scope) && SCOPES.includes(scope as GraphScope) ? (scope as GraphScope) : 'auto',
    relation: isString(relation) && RELATIONS.includes(relation as GraphRelation) ? (relation as GraphRelation) : 'follow',
    ...(isString(raw['degraded']) ? { degraded: raw['degraded'] } : {}),
  };
}
