PKG_VERSION = $(shell cargo pkgid | cut -d"#" -f2)
IMG_NAME = aleksxor/rango-load-tester

all: docker_build docker_push

docker_build:
	docker build -t $(IMG_NAME) .
	docker tag $(IMG_NAME):latest $(IMG_NAME):$(PKG_VERSION)

docker_push:
	docker push $(IMG_NAME):latest
	docker push $(IMG_NAME):$(PKG_VERSION)
