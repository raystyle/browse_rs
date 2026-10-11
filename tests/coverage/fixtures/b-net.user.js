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
    if (op === 'xhr-get') {
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
    } else if (op === 'alive') {
      out({ alive: true });
    } else { out({}); }
  } catch (e) { out({ fatal: String(e) }); }
})();
