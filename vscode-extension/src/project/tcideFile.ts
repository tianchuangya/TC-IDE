/**
 * Reading and writing the `.tcIDE` project document.
 *
 * The document is the project's own description: what to build, what the graph
 * shows, what to leave out. Machine specific state - window size, scroll
 * position, the last selected node - does not belong here, and neither does a
 * secret; the handoff forbids both in as many words.
 */

import * as path from 'node:path';

export const FILE_EXTENSION = 'tcIDE';
export const SCHEMA_VERSION = 1;

export interface ProjectDocument {
  schemaVersion: number;
  project: { name: string; root: string };
  semanticGraph: {
    defaultScope: 'auto' | 'project' | 'folder' | 'file' | 'symbol';
    relation: 'follow' | 'callers' | 'callees';
    excluded: string[];
  };
  tasks: { build: string; run: string; package: string };
  packageTargets: string[];
}

/** The document a new project starts with. */
export function defaultDocument(name: string): ProjectDocument {
  return {
    schemaVersion: SCHEMA_VERSION,
    project: { name, root: '.' },
    semanticGraph: {
      defaultScope: 'auto',
      relation: 'follow',
      excluded: ['**/node_modules/**', '**/target/**', '**/.git/**'],
    },
    tasks: { build: 'build', run: 'run', package: 'package' },
    packageTargets: [],
  };
}

/** The conventional path of a project's document. */
export function documentPath(root: string, name?: string): string {
  const folder = name ?? path.basename(root) ?? 'project';
  return path.join(root, `${folder}.${FILE_EXTENSION}`);
}

/**
 * Parse a document, reporting what was wrong instead of guessing.
 *
 * An unknown field is ignored so a newer document still opens; a wrong type is
 * an error, because silently substituting a default would change what the
 * project builds without saying so.
 */
export function parseDocument(text: string): { document: ProjectDocument; warnings: string[] } | { error: string } {
  let raw: unknown;
  try {
    raw = JSON.parse(text) as unknown;
  } catch (error) {
    return { error: `不是有效的 JSON：${error instanceof Error ? error.message : String(error)}` };
  }
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return { error: '顶层必须是一个对象' };
  }
  const record = raw as Record<string, unknown>;
  const warnings: string[] = [];

  const version = record['schemaVersion'];
  if (typeof version !== 'number' || !Number.isFinite(version)) {
    return { error: '缺少 schemaVersion' };
  }
  if (version > SCHEMA_VERSION) {
    return { error: `文档版本 ${version} 高于本扩展支持的 ${SCHEMA_VERSION}` };
  }

  const document = defaultDocument('project');

  const project = record['project'];
  if (typeof project === 'object' && project !== null) {
    const name = (project as Record<string, unknown>)['name'];
    const root = (project as Record<string, unknown>)['root'];
    if (typeof name === 'string' && name.length > 0) {
      document.project.name = name;
    }
    if (typeof root === 'string' && root.length > 0) {
      document.project.root = root;
    }
  }

  const graph = record['semanticGraph'];
  if (typeof graph === 'object' && graph !== null) {
    const g = graph as Record<string, unknown>;
    const scope = g['defaultScope'];
    if (typeof scope === 'string' && ['auto', 'project', 'folder', 'file', 'symbol'].includes(scope)) {
      document.semanticGraph.defaultScope = scope as ProjectDocument['semanticGraph']['defaultScope'];
    } else if (scope !== undefined) {
      warnings.push('defaultScope 不是已知范围，已使用 auto');
    }
    const relation = g['relation'];
    if (typeof relation === 'string' && ['follow', 'callers', 'callees'].includes(relation)) {
      document.semanticGraph.relation = relation as ProjectDocument['semanticGraph']['relation'];
    } else if (relation !== undefined) {
      warnings.push('relation 不是已知关系，已使用 follow');
    }
    const excluded = g['excluded'];
    if (Array.isArray(excluded)) {
      document.semanticGraph.excluded = excluded.filter((entry): entry is string => typeof entry === 'string');
    }
  }

  const tasks = record['tasks'];
  if (typeof tasks === 'object' && tasks !== null) {
    const t = tasks as Record<string, unknown>;
    for (const key of ['build', 'run', 'package'] as const) {
      const value = t[key];
      if (typeof value === 'string' && value.length > 0) {
        document.tasks[key] = value;
      }
    }
  }

  const targets = record['packageTargets'];
  if (Array.isArray(targets)) {
    document.packageTargets = targets.filter((entry): entry is string => typeof entry === 'string');
  }

  // A field the extension does not know is reported rather than kept, so a
  // typo does not look like a setting that works.
  const known = new Set(['schemaVersion', 'project', 'semanticGraph', 'tasks', 'packageTargets']);
  for (const key of Object.keys(record)) {
    if (!known.has(key)) {
      warnings.push(`未知字段 ${key} 已忽略`);
    }
  }

  return { document, warnings };
}

/** Serialise a document for writing. */
export function serialiseDocument(document: ProjectDocument): string {
  return `${JSON.stringify(document, null, 2)}\n`;
}
