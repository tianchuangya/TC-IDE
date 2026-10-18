/**
 * The webview is a separate document, so everything it sends is untrusted
 * input. These tests are the ones that keep a malformed or hostile message
 * from reaching anything that acts on it.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  isAllowedUri,
  parseGraphToHost,
  parseNode,
  parseSnapshot,
  PROTOCOL_VERSION,
  RELATIONS,
  SCOPES,
} from '../graph/protocol';

test('a well formed message is accepted', () => {
  assert.deepEqual(parseGraphToHost({ version: 1, type: 'ready' }), { version: 1, type: 'ready' });
  assert.deepEqual(parseGraphToHost({ version: 1, type: 'refresh' }), { version: 1, type: 'refresh' });
  assert.deepEqual(parseGraphToHost({ version: 1, type: 'changeScope', scope: 'file' }), {
    version: 1,
    type: 'changeScope',
    scope: 'file',
  });
  assert.deepEqual(parseGraphToHost({ version: 1, type: 'changeRelation', relation: 'callers' }), {
    version: 1,
    type: 'changeRelation',
    relation: 'callers',
  });
  assert.deepEqual(parseGraphToHost({ version: PROTOCOL_VERSION, type: 'fold', nodeId: 'a', folded: true }), {
    version: 1,
    type: 'fold',
    nodeId: 'a',
    folded: true,
  });
});

test('the parser refuses what it does not recognise', () => {
  // An unknown type, a missing version, a wrong version and a version sent as
  // a string are all refused rather than coerced.
  assert.equal(parseGraphToHost({ version: 1, type: 'launchMissiles' }), null);
  assert.equal(parseGraphToHost({ type: 'ready' }), null);
  assert.equal(parseGraphToHost({ version: 2, type: 'ready' }), null);
  assert.equal(parseGraphToHost({ version: '1', type: 'ready' }), null);
  assert.equal(parseGraphToHost(null), null);
  assert.equal(parseGraphToHost('ready'), null);
  assert.equal(parseGraphToHost(42), null);
});

test('a scope or relation outside the fixed set is refused', () => {
  assert.equal(parseGraphToHost({ version: 1, type: 'changeScope', scope: 'everything' }), null);
  assert.equal(parseGraphToHost({ version: 1, type: 'changeScope', scope: 7 }), null);
  assert.equal(parseGraphToHost({ version: 1, type: 'changeRelation', relation: 'both' }), null);
  // Every listed value is accepted, so the list and the parser cannot drift.
  for (const scope of SCOPES) {
    assert.notEqual(parseGraphToHost({ version: 1, type: 'changeScope', scope }), null, scope);
  }
  for (const relation of RELATIONS) {
    assert.notEqual(parseGraphToHost({ version: 1, type: 'changeRelation', relation }), null, relation);
  }
});

test('a jump target has to be a real location', () => {
  const good = parseGraphToHost({
    version: 1,
    type: 'openLocation',
    uri: 'file:///home/me/src/main.rs',
    line: 41,
    column: 8,
  });
  assert.deepEqual(good, {
    version: 1,
    type: 'openLocation',
    uri: 'file:///home/me/src/main.rs',
    line: 41,
    column: 8,
  });
  // Negative and fractional positions are clamped rather than rejected: the
  // position is a hint, but the uri still has to be safe.
  const clamped = parseGraphToHost({
    version: 1,
    type: 'openLocation',
    uri: 'file:///tmp/a.rs',
    line: -5,
    column: 3.9,
  });
  assert.equal(clamped !== null && clamped.type === 'openLocation' ? clamped.line : null, 0);
  assert.equal(clamped !== null && clamped.type === 'openLocation' ? clamped.column : null, 3);
});

test('a uri with an unexpected scheme is refused', () => {
  // This is the message that could otherwise make the extension open
  // something it should not.
  for (const uri of [
    'command:workbench.action.terminal.new',
    'http://example.com/x',
    'javascript:alert(1)',
    'data:text/html,<script>',
    '',
    'x'.repeat(5000),
  ]) {
    assert.equal(isAllowedUri(uri), false, uri.slice(0, 40));
    assert.equal(
      parseGraphToHost({ version: 1, type: 'openLocation', uri, line: 1, column: 1 }),
      null,
      uri.slice(0, 40),
    );
  }
  // The schemes the extension does use are accepted.
  for (const uri of [
    'file:///tmp/a.rs',
    'untitled:Untitled-1',
    'vscode-remote://ssh-remote+host/tmp/a.rs',
    'vscode-notebook-cell:/x.ipynb#W1sZmlsZQ%3D%3D',
  ]) {
    assert.equal(isAllowedUri(uri), true, uri);
  }
});

test('a search query is bounded', () => {
  const long = parseGraphToHost({ version: 1, type: 'search', query: 'a'.repeat(1000) });
  assert.equal(long !== null && long.type === 'search' ? long.query.length : -1, 200);
  assert.equal(parseGraphToHost({ version: 1, type: 'search', query: 5 }), null);
});

test('a node with a bad level or no id is dropped', () => {
  const good = parseNode({
    id: 'symbol:src/main.rs:41:build',
    level: 'function',
    title: 'build()',
    subtitle: 'main.rs:41',
    depth: 2,
    hasChildren: false,
    folded: false,
  });
  assert.notEqual(good, null);
  assert.equal(good?.level, 'function');
  assert.equal(good?.depth, 2);

  // An unknown level, a missing id, an empty id and a non numeric depth are
  // all refused, so a bad node is dropped rather than drawn wrongly.
  assert.equal(parseNode({ ...good, level: 'planet' }), null);
  assert.equal(parseNode({ ...good, id: '' }), null);
  assert.equal(parseNode({ ...good, id: 42 }), null);
  assert.equal(parseNode({ ...good, depth: 'deep' }), null);
  assert.equal(parseNode({ ...good, title: 1 }), null);
  assert.equal(parseNode(null), null);
});

test('a node location is only kept when it is safe', () => {
  const base = {
    id: 'x',
    level: 'file',
    title: 'a.rs',
    subtitle: '',
    depth: 0,
    hasChildren: false,
    folded: false,
  };
  const safe = parseNode({ ...base, location: { uri: 'file:///tmp/a.rs', line: 3, column: 1 } });
  assert.deepEqual(safe?.location, { uri: 'file:///tmp/a.rs', line: 3, column: 1 });
  // An unsafe uri is dropped from the node, but the node itself survives: it
  // can still be listed, it just cannot be jumped to.
  const unsafe = parseNode({ ...base, location: { uri: 'command:x', line: 3, column: 1 } });
  assert.equal(unsafe?.location, undefined);
  assert.notEqual(unsafe, null);
});

test('a snapshot keeps the nodes that parse and reports what it dropped', () => {
  const snapshot = parseSnapshot({
    nodes: [
      { id: 'a', level: 'file', title: 'a.rs', subtitle: '', depth: 0, hasChildren: false, folded: false },
      { id: 'b', level: 'nonsense', title: 'b', subtitle: '', depth: 0, hasChildren: false, folded: false },
      { id: 'c', level: 'folder', title: 'src', subtitle: '2 files', depth: 0, hasChildren: true, folded: true },
    ],
    total: 10,
    scope: 'file',
    relation: 'follow',
  });
  assert.notEqual(snapshot, null);
  assert.equal(snapshot?.nodes.length, 2, 'the invalid node is dropped');
  assert.equal(snapshot?.total, 10, 'the total is what the extension said, not what fitted');
  assert.equal(snapshot?.nodes[1]?.folded, true);

  // A snapshot without a node list is refused entirely.
  assert.equal(parseSnapshot({ total: 3 }), null);
  assert.equal(parseSnapshot(null), null);

  // An unknown scope falls back rather than failing the whole snapshot.
  const fallback = parseSnapshot({ nodes: [], scope: 'galaxy', relation: 'orbit' });
  assert.equal(fallback?.scope, 'auto');
  assert.equal(fallback?.relation, 'follow');
});
