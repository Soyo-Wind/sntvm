# Sinot VM

``` mermaid
flowchart TD
    A[variable x=0 time:0] -->B[[branch x]]
    B --> C([potential b1 x=1])
    B --> D([potential b2 x=2])
    B --> E([potential b3 x=3]) 
    C --> F
    D ==> F[[merge b2]]
    E --> F
    F ==> G[variable x=2 time:1]
```

## 概要
- 独自スクリプト言語の実行環境
- サポート：
  - 変数(let)
  - ブランチ(branch)・マージ(merge)
  - 入力(input)
  - 出力(print)
  - ListPush / SetInsert
  - Floatラップ対応
  - 空リスト/空セット対応
  
