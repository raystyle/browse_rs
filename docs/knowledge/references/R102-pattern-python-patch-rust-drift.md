---
version: 1
links:
  - target: sources/diary/2026-10-10-con02三层建与六代同步
    relation: derived_from
  - target: sources/diary/2026-09-19-issue-batch-review
    relation: derived_from
---

# R102 模式:python 批改 fmt 过的 Rust 源锚失配,批改三连坑

## PROBLEM

用 python heredoc 里的 str.replace 给已被 rustfmt 重排过的 Rust 源打补丁:引号转义断裂、漏右括、锚文本与 fmt 后实文不一致,replace 静默 no-op 或写坏文件;最重一例 open(w) 截断后才在拼串处抛错,client.rs 被清空,靠 git checkout 救回。

## ROOT CAUSE

锚的真相随 fmt 漂移:rustfmt 每轮重排缩进与换行,昨天匹配的锚今天失配;python replace 对失配零报错(静默 no-op),而 open(path,'w') 在写前即截断文件,错误发生在拼串阶段时损害已落盘。

## FIX

fmt 过的源一律用 Read+Edit 工具(锚失配即报错不静默);python 批改必须先拼串再 open(w) 且 replace 带 assert 计数。

## Trace

- sources/diary/2026-10-10-con02三层建与六代同步(坑节:五犯实档+client.rs 清空案)
- sources/diary/2026-09-19-issue-batch-review(首犯三连坑入册)
- commit:a428d76(本批五犯现场)、514b3cc(#81 批终修)
