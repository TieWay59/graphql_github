# 数据采集

- 结合 graphql 框架

  - 放弃 [Introspecting an API - Cynic - A GraphQL Client For Rust (cynic-rs.dev)](https://cynic-rs.dev/schemas/introspection) 设计的太过复杂。

  schemas 可以直接下载：[Public schema - GitHub Docs](https://docs.github.com/en/graphql/overview/public-schema)

## 版本说明

- 0.0.1

  - [x] 搭建基本的三类任务查询。
    - [x] discussions
    - [x] pr
    - [x] issues
  - [x] 查询结果先按照 json 文件的形式，以仓库为文件夹存储。（咱不考虑数据库）
    - 命名格式参考： `<owner>/<repo>/discussions/[page_number].json`

- 0.0.2

  - [ ] 寻找合适的数据库列表进行采集。
    - [Open Leaderboard (x-lab.info)](https://open-leaderboard.x-lab.info/)
