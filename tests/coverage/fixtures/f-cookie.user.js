// ==UserScript==
// @name         cv-f-cookie
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @grant        GM_cookie
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  var BASE = 'http://127.0.0.1:18871';
  function out(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('probe', JSON.stringify(o)); }
  function names(cs) { return cs ? cs.map(function (c) { return c.name; }) : null; }
  try {
    if (op === 'alive') {
      out({ alive: true });
    } else if (op === 'ck-raw') {
      var mode = (q.get('payload') || '').replace(/"/g, '');
      var det = { url: BASE + '/' };
      var which = (mode === 'list' || mode === 'set' || mode === 'del') ? mode : 'list';
      if (which === 'set') det = { url: BASE + '/', name: 'cvg', value: 'g1' };
      if (which === 'del') det = { url: BASE + '/', name: 'cvg' };
      GM_cookie(which, det, function () {
        var a = [];
        for (var i = 0; i < arguments.length; i++) {
          var v = arguments[i];
          a.push({ t: Array.isArray(v) ? 'array:' + v.length : (v === null ? 'null' : typeof v),
                   s: typeof v === 'object' && v !== null ? JSON.stringify(v).slice(0, 100) : String(v).slice(0, 100) });
        }
        out({ which: which, argc: arguments.length, args: a });
      });
      setTimeout(function () { out({ which: which, stalled: true }); }, 3000);
    } else if (op === 'ck-raw-promise') {
      (GM.cookie && GM.cookie.list ? GM.cookie.list({ url: BASE + '/' }) : Promise.reject(new Error('no GM.cookie')))
        .then(function (v) { out({ t: Array.isArray(v) ? 'array:' + v.length : typeof v, s: JSON.stringify(v).slice(0, 120) }); })
        .catch(function (e) { out({ perr: String(e) }); });
      setTimeout(function () { out({ stalled: true }); }, 3000);
    } else if (op === 'ck-list') {
      GM_cookie('list', { url: BASE + '/' }, function (err, cookies) {
        GM_cookie('list', { url: BASE + '/', name: 'cvp' }, function (e2, f) {
          out({ err: err ? String(err) : null, n: cookies ? cookies.length : -1, names: names(cookies),
                filtN: f ? f.length : -1 });
        });
      });
      setTimeout(function () { out({ stalled: true }); }, 3000);
    } else if (op === 'ck-set') {
      GM_cookie('set', { url: BASE + '/', name: 'cvg', value: 'g1' }, function (err) {
        GM_cookie('list', { url: BASE + '/' }, function (e2, cookies) {
          out({ setErr: err ? String(err) : null, hasCvg: cookies ? names(cookies).indexOf('cvg') >= 0 : false,
                docCk: document.cookie });
        });
      });
      setTimeout(function () { out({ stalled: true }); }, 3000);
    } else if (op === 'ck-del') {
      GM_cookie('set', { url: BASE + '/', name: 'cvg', value: 'g1' }, function () {
        GM_cookie('delete', { url: BASE + '/', name: 'cvg' }, function (err) {
          GM_cookie('list', { url: BASE + '/' }, function (e2, cookies) {
            out({ delErr: err ? String(err) : null, stillCvg: cookies ? names(cookies).indexOf('cvg') >= 0 : null });
          });
        });
      });
      setTimeout(function () { out({ stalled: true }); }, 3500);
    } else if (op === 'ck-gate') {
      GM_cookie('set', { url: 'https://example.org/', name: 'cvx', value: 'x' }, function (err) {
        out({ gateErr: err ? String(err) : null, refused: !!err });
      });
      setTimeout(function () { out({ stalled: true }); }, 3000);
    } else if (op === 'ck-promise') {
      (GM.cookie && GM.cookie.list ? GM.cookie.list({ url: BASE + '/' }) : Promise.reject(new Error('no GM.cookie')))
        .then(function (cookies) { out({ pn: cookies ? cookies.length : -1, names: names(cookies) }); })
        .catch(function (e) { out({ perr: String(e) }); });
      setTimeout(function () { out({ stalled: true }); }, 3000);
    }
  } catch (e) { out({ fatal: String(e) }); }
})();
