/**
 * Boots the webview with a fixed scenario.
 *
 * The webview normally waits for messages from the Extension Host. The harness
 * cannot send those, so it installs a stub `acquireVsCodeApi` and then feeds
 * the webview the same messages the host would, in the same order. That keeps
 * the harness honest: it exercises the real message handling rather than a
 * shortcut that only exists for screenshots.
 */

(function bootHarness() {
  const boot = window.__TC_HARNESS__;
  if (!boot) {
    return;
  }
  const scenario = boot.scenario;
  const tokens = boot.tokens;
  const sent = [];

  // Record what the view tries to send, so a geometry check can assert that a
  // click produced a message instead of assuming it did.
  window.__TC_SENT__ = sent;
  window.acquireVsCodeApi = function acquireVsCodeApi() {
    return {
      postMessage(message) {
        sent.push(message);
      },
      getState() {
        return undefined;
      },
      setState() {
        return undefined;
      },
    };
  };

  const deliver = (message) => {
    window.postMessage(message, '*');
  };

  window.addEventListener('load', () => {
    // The webview posts `ready` on load; answer with the theme first, then the
    // content, which is the order the provider uses.
    setTimeout(() => {
      deliver({ version: 1, type: 'setTheme', theme: tokens });
      deliver({ version: 1, type: 'setReducedMotion', reduced: scenario.reducedMotion === true });
      // The explicit state wins over the snapshot: an error scenario has no
      // snapshot, and testing that first sent it down the empty branch, which
      // made the error screenshot a byte copy of the empty one.
      if (scenario.state === 'loading') {
        deliver({ version: 1, type: 'setState', state: 'loading' });
      } else if (scenario.state === 'error') {
        deliver({ version: 1, type: 'setState', state: 'error', message: scenario.message ?? '' });
      } else if (scenario.state === 'empty' || scenario.snapshot === null) {
        deliver({ version: 1, type: 'setState', state: 'empty' });
      } else {
        deliver({ version: 1, type: 'replaceGraph', requestId: 'harness', graph: scenario.snapshot });
      }
      if (typeof scenario.search === 'string' && scenario.search.length > 0) {
        const input = document.getElementById('search-input');
        const row = document.getElementById('search-row');
        if (input && row) {
          row.hidden = false;
          input.value = scenario.search;
          input.dispatchEvent(new Event('input'));
        }
      }
      window.__TC_HARNESS_READY__ = true;
    }, 0);
  });
})();
