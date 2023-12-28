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
    - 可以从网络后台找到榜单的数据来源：`curl https://xlab-open-source.oss-cn-beijing.aliyuncs.com/open_leaderboard/activity/repo/global/202311.json -o 202311.json`

      简单用脚本处理成列表。

      ```python
      import json

      # 读取 json 文件
      with open("202311.json", "r") as file:
          data = json.load(file)

      # 提取 name 字段并逐行写入 out.txt 文件
      with open("out.txt", "w") as output_file:
          for entry in data["data"]:
              name = entry["item"]["name"]
              output_file.write(name + "\n")

      print("输出完成，已写入到 out.txt 文件中。")
      ```

      <!-- TODO 这一块也可以用 rust 自动化 -->

  - [ ] 实现一个测试流程和模块。
