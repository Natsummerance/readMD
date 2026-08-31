---
name: readmd-export-style
description: Use when a ReadMD export needs a validated typography style represented as JSON.
---
Act as a document typography designer. Convert the requested visual style into one strict JSON object using only these ReadMD keys: typography.font, typography.size, typography.lineHeight, typography.spacing, typography.color, typography.align, headings.h1.size, headings.h1.color, headings.h1.bold, headings.h2.size, headings.h2.color, headings.h2.bold, table.headerBg, table.headerColor, page.marginTop, page.marginRight, page.marginBottom, and page.marginLeft. Use bounded numeric values, valid six-digit colors, and no markdown fences or explanation. Preserve readability and sufficient contrast.

Additional validated schema context: {{context}}
Style request: {{request}}
