// ==UserScript==
// @name         cv-c-ui
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @grant        GM_notification
// @grant        GM_setClipboard
// @grant        GM_addStyle
// @grant        GM_addElement
// @grant        GM_audio
// @grant        GM_log
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  var BASE = 'http://127.0.0.1:18871';
  var ICON = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
  function out(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('probe', JSON.stringify(o)); window.__cv = o; }
  try {
    if (op === 'notify') {
      var h = GM_notification({ title: 'cv', text: 'cv-text', timeout: 700, image: ICON, icon: ICON },
        function (why) { out({ ndone: String(why), hasHandle: !!(h && (h.click || h.close)), icon: 'data' }); });
      setTimeout(function () { out({ ndone: 'STALLED' }); }, 4000);
    } else if (op === 'notify-click') {
      var ev = {};
      var h2 = GM_notification({ title: 'cv', text: 'clickme', timeout: 6000, image: ICON,
        onclick: function () { ev.clicked = true; },
        ondone: function (w) { ev.done = String(w); } });
      if (h2 && typeof h2.click === 'function') {
        setTimeout(function () { try { h2.click(); } catch (e) { ev.clickErr = String(e); } setTimeout(function () { out(ev); }, 400); }, 300);
      } else { setTimeout(function () { out({ noHandle: true }); }, 2500); }
    } else if (op === 'notify-http-icon') {
      GM_notification({ title: 'cv', text: 'httpicon', timeout: 700, image: BASE + '/icon.png' },
        function (why) { out({ ndone: String(why), icon: 'http' }); });
      setTimeout(function () { out({ ndone: 'STALLED', icon: 'http' }); }, 4000);
    } else if (op === 'clipboard') {
      var st = { pend: 2 };
      function fin() { if (--st.pend <= 0) delete st.pend, out(st); }
      try { GM_setClipboard('cv-clip-text', 'text', function (e) { st.text = e ? 'ERR:' + String(e && e.message || e) : 'ok'; fin(); }); }
      catch (e) { st.text = 'THROW:' + String(e); fin(); }
      try { GM_setClipboard('<b>cv</b>', 'html', function (e) { st.html = e ? 'ERR:' + String(e && e.message || e) : 'ok'; fin(); }); }
      catch (e) { st.html = 'THROW:' + String(e); fin(); }
      setTimeout(function () { st.stalled = true; out(st); }, 2500);
    } else if (op === 'addstyle') {
      GM_addStyle('.cv-x { color: rgb(12, 34, 56); }');
      var s = document.createElement('span'); s.className = 'cv-x'; s.textContent = 'x';
      document.body.appendChild(s);
      var has = Array.prototype.some.call(document.querySelectorAll('style'), function (st) { return st.textContent.indexOf('cv-x') >= 0; });
      out({ sheet: has, color: getComputedStyle(s).color });
    } else if (op === 'addelement') {
      var d = GM_addElement('div', { id: 'cv-ae', textContent: 'ael' });
      var v = {};
      try { GM_addElement(document.body, 'span', { id: 'cv-ae2', textContent: 'ae2' }); v.parentFirst = !!document.getElementById('cv-ae2'); }
      catch (e) { v.parentFirst = 'ERR:' + String(e); }
      try { GM_addElement('span', { id: 'cv-ae3', textContent: 'ae3' }, document.body); v.parentLast = !!document.getElementById('cv-ae3'); }
      catch (e) { v.parentLast = 'ERR:' + String(e); }
      try { var m = GM_addElement('span', { id: 'cv-ae4' }); document.body.appendChild(m); v.manual = !!document.getElementById('cv-ae4'); }
      catch (e) { v.manual = 'ERR:' + String(e); }
      out({ attached: !!(d && d.parentNode && document.getElementById('cv-ae')), forms: v });
    } else if (op === 'audio') {
      var t = typeof GM_audio, v = {};
      var pre = document.createElement('audio'); pre.id = 'cv-audio-pre'; document.body.appendChild(pre);
      try { var a0 = GM_audio(); v.noarg = a0 ? (a0.play ? 'audio-el' : String(a0).slice(0, 40)) : 'null'; } catch (e) { v.noarg = 'ERR:' + String(e).slice(0, 80); }
      try { var a1 = GM_audio('#cv-audio-pre'); v.selector = a1 ? (a1.play ? 'audio-el' : String(a1).slice(0, 40)) : 'null'; } catch (e) { v.selector = 'ERR:' + String(e).slice(0, 80); }
      try { var a2 = GM_audio({ url: BASE + '/silence.wav' }); v.obj = a2 ? (a2.play ? 'audio-el' : String(a2).slice(0, 40)) : 'null'; } catch (e) { v.obj = 'ERR:' + String(e).slice(0, 80); }
      try { var a3 = GM_audio('wav', BASE + '/silence.wav'); v.mime = a3 ? (a3.play ? 'audio-el' : String(a3).slice(0, 40)) : 'null'; } catch (e) { v.mime = 'ERR:' + String(e).slice(0, 80); }
      out({ type: t, forms: v });
    } else if (op === 'log') {
      GM_log('CVLOG_MARKER_7f3a');
      out({ logged: true });
    } else if (op === 'alive') {
      out({ alive: true });
    } else { out({}); }
  } catch (e) { out({ fatal: String(e) }); }
})();
