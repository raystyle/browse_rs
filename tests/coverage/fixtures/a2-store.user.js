// ==UserScript==
// @name         cv-a2-store
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @grant        GM_setValue
// @grant        GM_getValue
// @grant        GM_listValues
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  function out(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('probe2', JSON.stringify(o)); }
  try {
    if (op === 'cross-list') {
      out({ list: GM_listValues() });
    } else if (op === 'cross-write') {
      GM_setValue('a2k', { who: 'a2' });
      out({ list: GM_listValues() });
    }
  } catch (e) { out({ fatal: String(e) }); }
})();
