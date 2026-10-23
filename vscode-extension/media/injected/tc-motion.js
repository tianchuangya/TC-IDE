/* TC-IDE enhanced motion layer. No network, eval, or workspace access. */
(() => {
  'use strict';
  if (window.__tcIdeMotion?.dispose) window.__tcIdeMotion.dispose();

  const layer = document.createElement('div');
  layer.id = 'tcide-motion-layer';
  layer.setAttribute('aria-hidden', 'true');
  document.body.appendChild(layer);

  const disposers = [];
  const listen = (target, type, handler, options) => {
    target.addEventListener(type, handler, options);
    disposers.push(() => target.removeEventListener(type, handler, options));
  };
  const later = (node, delay) => setTimeout(() => node.remove(), delay);
  const editorUnder = (target) => target instanceof Element ? target.closest('.monaco-editor') : null;
  const activeCursor = () => {
    const cursor = document.querySelector('.monaco-editor.focused .cursors-layer > .cursor');
    if (!(cursor instanceof HTMLElement)) return null;
    const rect = cursor.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0 ? rect : null;
  };

  let pointerFrame = 0;
  let pointer = { x: -100, y: -100 };
  listen(document, 'pointermove', (event) => {
    if (!editorUnder(event.target) || Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) < 11) return;
    pointer = { x: event.clientX, y: event.clientY };
    if (pointerFrame) return;
    pointerFrame = requestAnimationFrame(() => {
      pointerFrame = 0;
      const dot = document.createElement('i');
      dot.className = 'tcide-pointer-trail';
      dot.style.left = `${pointer.x}px`;
      dot.style.top = `${pointer.y}px`;
      layer.appendChild(dot);
      later(dot, 420);
    });
  }, true);

  let previousCursor = null;
  const sampleCursor = (lineBreak = false) => requestAnimationFrame(() => requestAnimationFrame(() => {
    const next = activeCursor();
    if (!next) return;
    if (previousCursor && (Math.abs(next.left - previousCursor.left) > .5 || Math.abs(next.top - previousCursor.top) > .5)) {
      const ghost = document.createElement('i');
      ghost.className = 'tcide-caret-ghost';
      ghost.style.left = `${previousCursor.left}px`;
      ghost.style.top = `${previousCursor.top}px`;
      ghost.style.width = `${Math.max(2, previousCursor.width)}px`;
      ghost.style.height = `${previousCursor.height}px`;
      layer.appendChild(ghost);
      later(ghost, 290);
    }
    if (lineBreak || (previousCursor && Math.abs(next.top - previousCursor.top) > next.height * .5)) {
      const ripple = document.createElement('i');
      ripple.className = 'tcide-line-ripple';
      ripple.style.left = `${next.right + 5}px`;
      ripple.style.top = `${next.top + next.height - 2}px`;
      layer.appendChild(ripple);
      later(ripple, 390);
    }
    previousCursor = next;
  }));

  listen(document, 'keydown', (event) => {
    if (!editorUnder(event.target)) return;
    if (!previousCursor) previousCursor = activeCursor();
    sampleCursor(event.key === 'Enter');
  }, true);
  listen(document, 'mousedown', (event) => {
    if (!editorUnder(event.target)) return;
    previousCursor = activeCursor();
  }, true);
  listen(document, 'mouseup', (event) => {
    if (editorUnder(event.target)) sampleCursor(false);
  }, true);

  window.__tcIdeMotion = {
    dispose() {
      if (pointerFrame) cancelAnimationFrame(pointerFrame);
      for (const dispose of disposers) dispose();
      layer.remove();
      delete window.__tcIdeMotion;
    },
  };
})();
