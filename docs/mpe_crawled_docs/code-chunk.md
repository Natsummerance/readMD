# 交互式代码块 (Code Chunk)

允许在预览中就地执行代码并回填图表与输出：

## 语法格式
```python {cmd=true id="plot1" matplotlib=true}
import matplotlib.pyplot as plt
import numpy as np

x = np.linspace(0, 10, 100)
plt.plot(x, np.sin(x))
plt.title("正弦波曲线")
plt.show()
```

## 核心控制属性
- `cmd=true`: 启用执行
- `cmd=true`: 隐藏源码，只展示结果
- `cmd=true`: 输出通道
- `cmd=true`: 会话延续（跨代码块共享变量）
