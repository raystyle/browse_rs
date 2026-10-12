// ==UserScript==
// @name         cv-b-net
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @connect      127.0.0.1
// @grant        GM_xmlhttpRequest
// @grant        GM.xmlHttpRequest
// @grant        GM_download
// @grant        GM_webRequest
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  var BASE = 'http://127.0.0.1:18871';
  function out(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('probe', JSON.stringify(o)); window.__cv = o; }
  try {
    if (location.pathname.indexOf('/blocked-nav') === 0) {
      // 规则若拦导航则此页不达;达则自报(B30c)
      out({ path: location.pathname, landed: document.body ? document.body.innerText.slice(0, 20) : '' });
    } else if (op === 'xhr-get') {
      GM_xmlhttpRequest({
        method: 'GET', url: BASE + '/fixture?b=net-get',
        onload: function (r) { out({ status: r.status, text: (r.responseText || '').slice(0, 40), hdr: (r.responseHeaders || '').indexOf('X-Cv') >= 0 }); },
        onerror: function (e) { out({ err: 'onerror' }); }
      });
    } else if (op === 'xhr-post') {
      GM_xmlhttpRequest({
        method: 'POST', url: BASE + '/echo', headers: { 'X-Cv-Custom': 'yes' }, data: 'payload=123',
        onload: function (r) { out({ status: r.status, echo: (r.responseText || '').slice(0, 60) }); },
        onerror: function () { out({ err: 'onerror' }); }
      });
    } else if (op === 'xhr-timeout') {
      GM_xmlhttpRequest({
        method: 'GET', url: BASE + '/slow', timeout: 600,
        onload: function (r) { out({ late: r.status }); },
        ontimeout: function () { out({ timedOut: true }); },
        onerror: function () { out({ err: 'onerror' }); }
      });
    } else if (op === 'xhr-types') {
      var want = ['text', 'json', 'document', 'blob'];
      var got = {};
      var pend = want.length;
      want.forEach(function (t) {
        GM_xmlhttpRequest({
          method: 'GET', url: BASE + '/t-' + t, responseType: t,
          onload: function (r) {
            var v = r.response;
            got[t] = (t === 'json') ? JSON.stringify(v) : (t === 'document') ? (v && v.title ? 'doc' : String(v)) : (t === 'blob') ? (v && v.size >= 0 ? 'blob' + v.size : String(v)) : String(v).slice(0, 20);
            if (--pend === 0) out(got);
          },
          onerror: function () { got[t] = 'ERR'; if (--pend === 0) out(got); }
        });
      });
    } else if (op === 'xhr-abort') {
      var ctrl = GM_xmlhttpRequest({ method: 'GET', url: BASE + '/slow', timeout: 8000, onload: function (r) { out({ late: true }); }, onerror: function (e) { out({ aborted: (e && e.error === 'aborted') || 'err:' + (e && e.error) }); } });
      setTimeout(function () { try { ctrl.abort(); setTimeout(function () { out({ abortCalled: true, late: window.__cv && window.__cv.late }); }, 900); } catch (e) { out({ abortThrew: String(e) }); } }, 250);
    } else if (op === 'xhr-promise') {
      GM.xmlHttpRequest({ method: 'GET', url: BASE + '/fixture?b=promise' }).then(function (r) { out({ status: r.status }); }).catch(function (e) { out({ err: String(e) }); });
    } else if (op === 'xhr-connect-deny') {
      GM_xmlhttpRequest({ method: 'GET', url: 'http://192.0.2.1:1/x', timeout: 1500, onload: function (r) { out({ leaked: r.status }); }, onerror: function () { out({ denied: true }); }, ontimeout: function () { out({ denied: 'timeout' }); } });
    } else if (op === 'download') {
      var dl = GM_download({ url: BASE + '/fixture?b=dl', name: 'cv-dl.txt', saveAs: false, onload: function () { out({ dlDone: true }); }, onerror: function (e) { out({ dlErr: (e && e.error) || 'err' }); } });
      out({ hasCtrl: !!(dl && (dl.abort || dl.pause)) });
    } else if (op === 'wr-cancel') {
      GM_webRequest('http://127.0.0.1:18871/blocked-*', { cancel: true }, function (info) { out({ blocked: info && info.blocked, err: info && info.error }); setTimeout(function () { out({ phase2: true }); }, 100); });
      var f = document.createElement('iframe');
      f.src = BASE + '/blocked-target';
      f.onload = function () { setTimeout(function () { out({ frameLoaded: f.contentDocument ? 'loaded' : 'denied' }); }, 200); };
      document.body.appendChild(f);
      setTimeout(function () { out({ timeoutNoCb: true }); }, 1500);
    } else if (op === 'xhr-semantics') {
      // v2 项 15 语义分界:404/500 走 onload 非 onerror;onerror 仅网络层;onloadend 恒火
      var st = {};
      function mk(label, url, timeout) {
        var e = { onload: 0, onerror: 0, onloadend: 0, prog: 0, status: null };
        st[label] = e;
        GM_xmlhttpRequest({
          method: 'GET', url: url, timeout: timeout || 0,
          onload: function (r) { e.onload++; e.status = r.status; },
          onerror: function (r) { e.onerror++; e.errStatus = r && r.status; },
          onloadend: function () { e.onloadend++; },
          onprogress: function (p) { e.prog++; if (p && p.loaded) e.loaded = true; }
        });
      }
      mk('r404', BASE + '/err404');
      mk('r500', BASE + '/err500');
      mk('netfail', 'http://127.0.0.1:9/x', 4000);
      mk('ok', BASE + '/fixture?b=sem');
      setTimeout(function () { out(st); }, 3600);
    } else if (op === 'xhr-redirect') {
      GM_xmlhttpRequest({
        method: 'GET', url: BASE + '/r302',
        onload: function (r) { out({ status: r.status, finalUrl: r.finalUrl, followed: r.finalUrl !== BASE + '/r302' }); },
        onerror: function () { out({ err: 'onerror' }); }
      });
    } else if (op === 'big') {
      var b = { pend: 2 };
      function fin() { if (--b.pend === 0) out(b); }
      function mk(key, mb) {
        GM_xmlhttpRequest({ method: 'GET', url: BASE + '/big?mb=' + mb, responseType: 'arraybuffer', timeout: 90000,
          onload: function (r) { b[key] = (r.response && r.response.byteLength) || ('str:' + String(r.response || '').length); fin(); },
          onerror: function (e) { b[key] = 'ERR:' + ((e && e.error) || 'net'); fin(); } });
      }
      mk('b32', 32); mk('b34', 34);
      setTimeout(function () { b.stalled = true; out(b); }, 95000);
    } else if (op === 'anon') {
      var a = { pend: 2 };
      function fin2() { if (--a.pend === 0) out(a); }
      GM_xmlhttpRequest({ method: 'GET', url: BASE + '/ckprobe', onload: function (r) { a.plain = JSON.parse(r.responseText).cookie; fin2(); }, onerror: function () { a.plain = 'ERR'; fin2(); } });
      GM_xmlhttpRequest({ method: 'GET', url: BASE + '/ckprobe', anonymous: true, onload: function (r) { a.anon = JSON.parse(r.responseText).cookie; fin2(); }, onerror: function () { a.anon = 'ERR'; fin2(); } });
      setTimeout(function () { a.stalled = true; out(a); }, 4000);
    } else if (op === 'dlctl') {
      var d = { prog: 0, duringPause: 0 };
      var inPause = false;
      var h = GM_download({ url: BASE + '/dlslow', name: 'cv-dlslow.bin', saveAs: false,
        onprogress: function (p) { d.prog++; if (inPause) d.duringPause++; if (p && p.loaded) d.loaded = p.loaded; },
        onload: function () { d.done = true; out(d); },
        onerror: function (e) { d.err = (e && e.error) || 'err'; out(d); } });
      d.hasCtrl = !!(h && h.pause && h.resume && h.abort);
      setTimeout(function () { try { h.pause(); inPause = true; d.pauseOk = true; } catch (e) { d.pauseErr = String(e); } }, 700);
      setTimeout(function () { inPause = false; try { h.resume(); d.resumeOk = true; } catch (e) { d.resumeErr = String(e); } }, 2100);
      setTimeout(function () { try { h.abort(); d.abortOk = true; } catch (e) { d.abortErr = String(e); } }, 3300);
      setTimeout(function () { if (!d.done && !d.err) { d.timeout = true; out(d); } }, 6000);
    } else if (op === 'wr2') {
      var w = { pend: 3 };
      function flush() { if (--w.pend === 0) setTimeout(function () { out(w); }, 400); }
      GM_webRequest([{ selector: { urls: [BASE + '/blocked-*'] }, action: { cancel: true } }], function (info) { w.ruleCb = info ? (info.error || String(info.blocked)) : 'null'; });
      var img = new Image();
      img.onerror = function () { w.imgBlocked = true; flush(); };
      img.onload = function () { w.imgBlocked = false; flush(); };
      img.src = BASE + '/blocked-img';
      fetch(BASE + '/blocked-fetch').then(function (r) { w.fetchOk = r.status; flush(); }, function (e) { w.fetchBlocked = String(e).slice(0, 50); flush(); });
      var f = document.createElement('iframe');
      f.src = BASE + '/blocked-frame';
      f.onload = function () { setTimeout(function () { w.frameDoc = f.contentDocument ? 'loaded' : 'denied'; flush(); }, 150); };
      f.onerror = function () { w.frameDoc = 'onerror'; flush(); };
      document.body.appendChild(f);
      setTimeout(function () { w.stalled = true; out(w); }, 5000);
    } else if (op === 'wr3') {
      var w3 = { pend: 3 };
      function fin3() { if (--w3.pend === 0) out(w3); }
      GM_webRequest([
        { selector: { urls: [BASE + '/redir-src'] }, action: { redirect: { url: BASE + '/redir-tgt' } } },
        { selector: { urls: [BASE + '/redir-src2'] }, action: { redirect: { url: 'http://127.0.0.1:18873/xorigin' } } },
        { selector: { urls: [BASE + '/redir-src3'] }, action: { redirect: { url: 'chrome://version' } } }
      ], function (info) { w3.ruleCb = info ? (info.error || 'ok') : 'null'; });
      var s1 = document.createElement('script');
      s1.src = BASE + '/redir-src';
      s1.onload = function () { w3.sameMarker = window.__wrMarker || 'none'; fin3(); };
      s1.onerror = function () { w3.sameMarker = 'net-err'; fin3(); };
      document.head.appendChild(s1);
      var s2 = document.createElement('script');
      s2.src = BASE + '/redir-src2';
      s2.onload = function () { w3.crossMarker = window.__wrMarker || 'none'; fin3(); };
      s2.onerror = function () { w3.crossMarker = 'net-err'; fin3(); };
      document.head.appendChild(s2);
      var s3 = document.createElement('script');
      s3.src = BASE + '/redir-src3';
      s3.onload = function () { w3.badMarker = window.__wrMarker || 'none'; fin3(); };
      s3.onerror = function () { w3.badMarker = 'net-err'; fin3(); };
      document.head.appendChild(s3);
      setTimeout(function () { w3.stalled = true; out(w3); }, 5000);
    } else if (op === 'wr-arm-hold') {
      GM_webRequest([{ selector: { urls: [BASE + '/blocked-*'] }, action: { cancel: true } }], function () {});
      out({ armed: true });
    } else if (op === 'wr-dl') {
      var dl = GM_download({ url: BASE + '/blocked-dl', name: 'cv-wrdl.txt', saveAs: false,
        onload: function () { out({ dlDone: true }); },
        onerror: function (e) { out({ dlErr: (e && e.error) || 'err' }); } });
      setTimeout(function () { out({ stalled: true }); }, 6000);
    } else if (op === 'alive') {
      out({ alive: true });
    }
  } catch (e) { out({ fatal: String(e) }); }
})();
