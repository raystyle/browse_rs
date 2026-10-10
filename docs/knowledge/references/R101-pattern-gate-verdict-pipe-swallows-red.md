---
version: 1
links:
  - target: sources/diary/2026-10-10-con02三层建与六代同步
    relation: derived_from
  - target: sources/diary/2026-10-08-全量评审修复批
    relation: derived_from
  - target: sources/diary/2026-09-22-artifact-structured-fields
    relation: derived_from
---

# R101 模式:门禁判定被尾管吞红,尾管吞红族

## PROBLEM

agent 跑门禁或长输出命令时,把判定接在管道尾(`cmd | tail`、`grep 旧日志`、`&& 链`),命令实际失败(退出码非 0、新错误已落新日志)而下游命令仍执行或 grep 命中旧文本,回执「绿」,假绿进 diary 与评审回执,自欺放大到下一轮。

## ROOT CAUSE

判定语义寄生在输出过滤上而非进程退出码上:管道的退出码默认取**最后一个**命令(tail/grep 恒 0/恒命中旧文件),红被吞;且「看见关键词」≠「本轮跑过」,日志文件是追加态,grep 旧日志命中的是历史不是本轮。

## FIX

门禁与长命令判定一律退出码形(`>log 2>&1; echo $?` 后人工看退出码),输出过滤与命令链分离,日志判定只认本轮新写的文件。

## Trace

- sources/diary/2026-10-10-con02三层建与六代同步(坑节:第三踩后立律)
- sources/diary/2026-10-08-全量评审修复批、2026-09-22-artifact-structured-fields(前两踩实档)
- commit:ad7f7ce(双评审期 grok 回执读不全案)、7475e48(PEVO 三踩封版批)
