// ==UserScript==
// @name         cv-d2-tab2
// @namespace    cv2
// @match        http://127.0.0.1:18871/*
// @grant        GM_getTab
// @grant        GM_saveTab
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  // 独立回执通道:只对 tab2-* op 写 probe2,不与主脚本 probe 互踩
  function out2(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('probe2', JSON.stringify(o)); }
  try {
    if (op === 'tab2-alive') {
      out2({ alive: true });
    } else if (op === 'tab2-read') {
      GM_getTab(function (d) {
        var first = (d === null || d === undefined) ? 'none' : ((d.n === undefined) ? 'isolated' : ('LEAK:' + JSON.stringify(d)));
        GM_saveTab({ who: 'd2' });
        GM_getTab(function (own) { out2({ first: first, own: own }); });
      });
      setTimeout(function () { out2({ stalled: true }); }, 4000);
    }
  } catch (e) { out2({ fatal: String(e) }); }
})();
