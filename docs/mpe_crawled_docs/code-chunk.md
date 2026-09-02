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
- `cmd` / `{cmd}`：启用受控执行；`cmd=python` 等值仅允许已声明的运行时别名。
- `hide=true` 或 `echo=false`：隐藏源码，只展示结果。
- `output=text|markdown|html|png|none`：选择输出通道；`output=true` 等同于文本输出并允许显式回写。
- `continue`：MPE 兼容标记，仅在宿主提供会话时启用；ReadMD 默认每个代码块独立沙箱执行。
