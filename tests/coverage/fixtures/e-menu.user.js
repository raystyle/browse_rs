// ==UserScript==
// @name         cv-e-menu
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @grant        GM_registerMenuCommand
// @grant        GM_unregisterMenuCommand
// @grant        GM_setValue
// @grant        GM_getValue
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  function out(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('probe', JSON.stringify(o)); }
  try {
    if (op === 'menu-reg') {
      GM_registerMenuCommand('cv-menu-alpha', function () { GM_setValue('menu-fired', 'alpha'); });
      GM_registerMenuCommand('cv-menu-beta', function () { GM_setValue('menu-fired', 'beta'); });
      out({ reg: 2 });
    } else if (op === 'menu-status') {
      out({ fired: GM_getValue('menu-fired', 'no') });
    } else if (op === 'menu-unreg') {
      var id = GM_registerMenuCommand('cv-menu-gamma', function () {});
      var u = GM_unregisterMenuCommand(id);
      out({ regThen: id !== undefined, unreg: u === undefined ? 'undef' : String(u) });
    } else if (op === 'menu-ak') {
      // v2 项 97:accessKey 形注册(触发链经管理页面板点击,CDP RunMenuCommand 面驱动侧另探)
      var ak = GM_registerMenuCommand('cv-menu-ak', function () { GM_setValue('ak-fired', '1'); }, 'k');
      out({ reg: ak !== undefined });
    } else if (op === 'menu-ak-status') {
      out({ fired: GM_getValue('ak-fired', 'no') });
    } else if (op === 'alive') {
      out({ alive: true });
    }
  } catch (e) { out({ fatal: String(e) }); }
})();
