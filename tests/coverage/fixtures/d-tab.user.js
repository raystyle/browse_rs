// ==UserScript==
// @name         cv-d-tab
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @grant        GM_openInTab
// @grant        GM_getTab
// @grant        GM_saveTab
// @grant        GM_getTabs
// @grant        GM_setValue
// @grant        GM_getValue
// @grant        GM_deleteValue
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  var BASE = 'http://127.0.0.1:18871';
  function out(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('probe', JSON.stringify(o)); window.__cv = o; }
  function stall(o, ms) { setTimeout(function () { o.stalled = true; out(o); }, ms || 4500); }
  try {
    if (op === 'alive') {
      out({ alive: true });
    } else if (op === 'open-handle') {
      var st = {};
      var h = GM_openInTab(BASE + '/?op=opened', { active: false });
      st.hasHandle = !!(h && (h.close || h.onload !== undefined));
      if (h) {
        try {
          var d = Object.getOwnPropertyDescriptor(h, 'onload') || {};
          st.onloadDesc = Object.keys(d).join(',') + (d.set === undefined ? ';noset' : ';set');
          st.onloadOwn = 'onload' in h;
        } catch (e) { st.descErr = String(e); }
        h.onload = function () { st.onloadFired = true; };
        h.onclose = function () { st.oncloseFired = true; };
        try { h.addEventListener && h.addEventListener('load', function () { st.loadEvt = true; }); st.hasAEL = !!h.addEventListener; }
        catch (e) { st.aelErr = String(e); }
      }
      setTimeout(function () {
        try { if (h) h.close(); } catch (e) { st.closeErr = String(e); }
        setTimeout(function () { st.closedField = h ? h.closed : null; st.hadOnloadAtClose = typeof (h && h.onload); out(st); }, 700);
      }, 1300);
      stall(st);
    } else if (op === 'open-active' || op === 'open-passive') {
      var active = op === 'open-active';
      GM_deleteValue('d-or');
      GM_openInTab(BASE + '/?op=opened', { active: active });
      var rep = null, t0 = Date.now();
      var iv = setInterval(function () {
        rep = GM_getValue('d-or', null);
        if (rep || Date.now() - t0 > 3000) {
          clearInterval(iv);
          out({ active: active, rep: rep, self: document.visibilityState });
        }
      }, 150);
      stall({ active: active }, 5000);
    } else if (op === 'opened' || op === 'opened-save') {
      GM_setValue('d-or', { vis: document.visibilityState, focus: document.hasFocus() });
      if (op === 'opened-save') GM_saveTab({ who: 'opened-tab' });
    } else if (op === 'tab-save') {
      GM_getTab(function (d) {
        GM_saveTab({ n: 42 });
        GM_getTab(function (d2) { out({ round: d2, prevType: d === null || d === undefined ? 'none' : typeof d }); });
      });
      stall({});
    } else if (op === 'tabs-agg') {
      GM_openInTab(BASE + '/?op=opened-save', { active: false });
      setTimeout(function () {
        GM_getTabs(function (m) {
          var ks = m ? Object.keys(m) : [];
          out({ cnt: ks.length, m: m });
        });
      }, 1600);
      stall({});
    } else if (op === 'tab-reread') {
      GM_getTab(function (d) { out({ d: d }); });
      stall({});
    }
  } catch (e) { out({ fatal: String(e) }); }
})();
