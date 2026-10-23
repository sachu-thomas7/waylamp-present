FROM python:3.12-slim
RUN pip install --no-cache-dir pymupdf==1.24.* python-pptx==1.0.*
WORKDIR /app
COPY analyze.py .
ENTRYPOINT ["python", "analyze.py"]
