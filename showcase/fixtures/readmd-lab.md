# 本地实验记录

## 阻尼振荡

```python cmd=true
import numpy as np

t = np.linspace(0, 8, 400)
y = np.exp(-0.45 * t) * np.cos(6 * t)
print(f"样本数={len(t)}, 峰值={y.max():.3f}")
```

## 结论

代码块保留在文档上下文中执行；输出只进入显示层，原始 `.md` 文件不变。
