# 验收与判定(browse 面配套)

> 判定纪律点名 [[gate-verdict]](退出码形,尾管吞红/旧日志假绿两防);本页收 browse 面的验收载体与红绿自测。

## google-search 验收实弹

载体:`tests/google-search.accept.ps1`(pwsh 7,browse_rs 仓):

- 10 次实弹,隔离(引擎每跑 `browse down` 重起 + 沉降 800ms 防竞态)+ 串行;瞬败空输出重试一次(竞态实录)
- 成功判据:results>=5 且 verdict=ok 且 drift=none;>=8/10 达标
- 判定:脚本退出码(0 过/1 未达标),pwsh 解析从首个 `{` 截断(stderr 提示行混流怪癖)
- 全量轨迹落 /tmp/google-search-accept/(run-N.json 逐次可查)

## 引擎与托管集体检

```bash
browse chrome doctor    # 在位/文件基线/pin;healthy 与 filesMatch 看
browse chrome list      # 版本与 pin 面
```

- 五端部署面变更后跑 doctor 对账(文件基线差 = 部署目录被外部改动或出包残留)

## workspace 门(check.ts)

```bash
cd <workspace 仓> && pnpm install && pnpm check && pnpm typecheck
```

- 对册在册数、detect 表驱动(vm 喂 facts)、prelude 运行时桩、vendor 钉版守卫(sha256 边车 + 字节等内嵌)

## 红线复述

- 一切门禁判定取退出码(`>log 2>&1; echo $?`),不接吞退出码管道
- 验收实录逐次留档(轨迹文件路径进 receipt),回执带证据自取自跑
