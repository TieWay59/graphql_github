# 数据采集

- 结合 graphql 框架

  - 放弃 [Introspecting an API - Cynic - A GraphQL Client For Rust (cynic-rs.dev)](https://cynic-rs.dev/schemas/introspection) 设计的太过复杂。

  schemas 可以直接下载：[Public schema - GitHub Docs](https://docs.github.com/en/graphql/overview/public-schema)

## 版本代办

- v0.0.1

  - 搭建基本的三类任务查询。
    - discussions
    - pr
    - issues
  - 查询结果先按照 json 文件的形式，以仓库为文件夹存储。（咱不考虑数据库）
    - 命名格式参考： `<owner>/<repo>/discussions/[page_number].json`

- v0.0.2

  - 增加请求失败重试的机制，并且加强了对请求失败的判断
  - 修复因为 reset 时间戳认知错误导致的意外
  - 寻找合适的数据库列表进行采集。

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

- v0.0.3

  - [x] 实现续爬
    - 就是在发生意外后，可以通过读取文件路径下的文件信息进行续爬。
    - 因为最后的 handle，编号，和 cursor 都是确定的。
    - 现在爬取应该趋于稳定。
  - [ ] 实现测试流程和模块。
  - [ ] 转移日志到文件。
