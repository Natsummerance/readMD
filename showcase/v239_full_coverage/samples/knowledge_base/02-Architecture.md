# ReadMD 架构设计规格

返回总览请跳转：[[01-System-Overview]]。

## 性能指标
本系统采用模块化按需加载机制，核心运行时纯离线零侵入。
关于伴读模块的集成机制，参见 [[04-Pet-Companion#Integration]]。
关于本地轻量数据库存储，参见 [[03-Database]]。

```
[前端 UI] <---> [Pywebview 统一网关] <---> [Python 业务内核]
                        |
            [SQLite WAL 索引引擎]
```
