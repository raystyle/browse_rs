// ==UserScript==
// @name         cv-a-store
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @grant        GM_setValue
// @grant        GM_getValue
// @grant        GM_deleteValue
// @grant        GM_listValues
// @grant        GM_addValueChangeListener
// @grant        GM_removeValueChangeListener
// @run-at       document-idle
// ==/UserScript==
(function () {
  // 驱动道:页面 ?op=<case>&payload=<json>;同步回执 attr probe(异步面用 window.__cv 缓冲 + poll op=status)
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  function out(o) {
    o = o || {};
    o.op = op;
    document.documentElement.setAttribute('probe', JSON.stringify(o));
    window.__cv = o;
  }
  try {
    if (op === 'roundtrip') {
      var p = JSON.parse(q.get('payload'));
      GM_setValue('k', p.v);
      out({ read: GM_getValue('k', '__MISS__') });
    } else if (op === 'del') {
      GM_setValue('a', 1); GM_setValue('b', 2);
      GM_deleteValue('a');
      out({ list: GM_listValues() });
    } else if (op === 'promise') {
      GM.setValue('pk', 'pv').then(function () {
        return GM.getValue('pk', '__MISS__');
      }).then(function (v) {
        return GM.deleteValue('pk').then(function () { out({ v: v }); });
      }).catch(function (e) { out({ err: String(e) }); });
    } else if (op === 'batch') {
      var b = { x: 1, y: 'two', z: [3] };
      GM.setValues(b).then(function () {
        return GM.getValues(['x', 'y', 'z', 'w']);
      }).then(function (got) {
        return GM.deleteValues(['x']).then(function () {
          return GM.listValues().then(function (rest) { out({ got: got, rest: rest }); });
        });
      }).catch(function (e) { out({ err: String(e) }); });
    } else if (op === 'optimistic') {
      // 回调形乐观读:写回调内立读新值
      GM_setValue('opt', 'old');
      GM_setValue('opt', 'new', function () {
        out({ immediate: GM_getValue('opt', '__MISS__') });
      });
    } else if (op === 'undef-del') {
      GM_setValue('u', 'here');
      GM_setValue('u', undefined);
      out({ list: GM_listValues(), gone: GM_getValue('u', '__GONE__') });
    } else if (op === 'listen-self') {
      GM_setValue('l', 0);
      var seen = [];
      GM_addValueChangeListener('l', function (name, oldV, newV, remote) {
        seen.push({ name: name, old: oldV, new: newV, remote: remote });
      });
      GM_setValue('l', 1); // 同帧写:同步放火 remote=false
      GM_setValue('l', 1); // 未变:跳过扇出
      setTimeout(function () { out({ seen: seen }); }, 300);
    } else if (op === 'listen-off') {
      var got2 = [];
      var tid = GM_addValueChangeListener('lo', function () { got2.push(1); });
      GM_setValue('lo', 'a');
      GM_removeValueChangeListener(tid);
      GM_setValue('lo', 'b');
      setTimeout(function () { out({ fires: got2.length }); }, 300);
    } else if (op === 'listen-cross-read') {
      // 跨 tab 面:只读收况(写侧在另一 tab 由驱动开 ?op=listen-cross-write)
      window.__cvCross = [];
      GM_addValueChangeListener('cx', function (name, oldV, newV, remote) {
        window.__cvCross.push({ old: oldV, new: newV, remote: remote });
      });
      out({ armed: true });
    } else if (op === 'listen-cross-status') {
      setTimeout(function () { out({ cross: window.__cvCross || [] }); }, 200);
    } else if (op === 'listen-cross-write') {
      GM_setValue('cx', 'from-other-tab');
      setTimeout(function () { out({ wrote: true, self: window.__cvCross || null }); }, 250);
    } else if (op === 'badjson-default') {
      // 值文件被外部写坏:脚本仍跑,读到默认值,写失败不留新文件(判读在驱动侧看盘)
      out({ v: GM_getValue('poison', '__DEFAULT__') });
    } else if (op === 'just-list') {
      out({ list: GM_listValues() });
    } else if (op === 'alive') {
      out({ alive: true });
    }
  } catch (e) {
    out({ fatal: String(e) });
  }
})();
