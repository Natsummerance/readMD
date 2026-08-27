# ReadMD 轻量级容器化与局域网 Web 镜像 (Alpine Python 3.11)
FROM python:3.11-alpine

WORKDIR /app

# 安装基础依赖
RUN apk add --no-cache \
    curl \
    ca-certificates \
    font-noto \
    font-noto-cjk

COPY config/requirements-linux.txt config/requirements.txt
RUN pip install --no-cache-dir --disable-pip-version-check -r config/requirements.txt || \
    pip install --no-cache-dir trafilatura bs4 markdown reportlab

COPY assets /app/assets
COPY src /app/src
COPY readmd.py /app/readmd.py
COPY VERSION /app/VERSION

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s CMD curl -f http://127.0.0.1:8080/ || exit 1

ENTRYPOINT ["python", "readmd.py", "--browser", "--port", "8080", "--share"]
