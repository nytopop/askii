NAME := $(shell cargo read-manifest | jq -r ".name")
VERSION := $(shell cargo read-manifest | jq -r ".version")
DESCRIPTION := $(shell cargo read-manifest | jq ".description")
AUTHOR := $(shell cargo read-manifest | jq ".authors[]")

DIST=dist
BIN=$(DIST)/bin/$(NAME)
DEB=$(DIST)/$(NAME)_$(VERSION)_amd64.deb
RPM=$(DIST)/$(NAME)-$(VERSION)-1.x86_64.rpm

.PHONY: all
all: $(BIN) $(DEB) $(RPM)

$(BIN):
	cargo build --release
	mkdir -p $(DIST)/bin
	cp target/release/$(NAME) $(BIN)

$(DEB): $(BIN)
	cd $(DIST) && fpm -s dir -t deb --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d libncurses6 -d libc6 --license MIT -f --deb-priority optional --deb-no-default-config-files bin/$(NAME)

$(RPM): $(BIN)
	cd $(DIST) && fpm -s dir -t rpm --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d "ncurses >= 6" --license MIT -f bin/$(NAME)

.PHONY: build
build: $(BIN)

.PHONY: distclean
distclean:
	rm -rf $(DIST)

.PHONY: clean
clean: distclean
	cargo clean

.PHONY: dev-clippy
dev-clippy:
	cargo watch -c -x clippy

.PHONY: dev-install
dev-install:
	cargo watch -c -x "install --path . --force"

.PHONY: install
install:
	cargo install --path . --force

.PHONY: release
release: all
	$(eval TOKEN := $(shell cat ~/.github-token-askii))
	cargo publish
	git tag -f v$(VERSION)
	git push --tags
	GITHUB_TOKEN=$(TOKEN) && gothub release --user nytopop --repo askii --tag v$(VERSION)
	GITHUB_TOKEN=$(TOKEN) && gothub upload --user nytopop --repo askii --tag v$(VERSION) --name $(BIN) --file $(BIN)
	GITHUB_TOKEN=$(TOKEN) && gothub upload --user nytopop --repo askii --tag v$(VERSION) --name $(DEB) --file $(DEB)
	GITHUB_TOKEN=$(TOKEN) && gothub upload --user nytopop --repo askii --tag v$(VERSION) --name $(RPM) --file $(RPM)
