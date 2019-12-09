NAME := $(shell cargo read-manifest | jq -r ".name")
VERSION := $(shell cargo read-manifest | jq -r ".version")
DESCRIPTION := $(shell cargo read-manifest | jq ".description")
AUTHOR := $(shell cargo read-manifest | jq ".authors[]")

DIST=dist

BIN=$(NAME)
DEB=$(NAME)_$(VERSION)_amd64.deb
RPM=$(NAME)-$(VERSION)-1.x86_64.rpm
PAC=$(NAME)-$(VERSION)-1-x86_64.pkg.tar.xz

BINPATH=$(DIST)/bin/$(BIN)
DEBPATH=$(DIST)/$(DEB)
RPMPATH=$(DIST)/$(RPM)
PACPATH=$(DIST)/$(PAC)

.PHONY: all
all: $(BINPATH) $(DEBPATH) $(RPMPATH) $(PACPATH)

$(BINPATH):
	cargo build --release
	mkdir -p $(DIST)/bin
	cp target/release/$(BIN) $(BINPATH)

$(DEBPATH): $(BINPATH)
	cd $(DIST) && fpm -s dir -t deb --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d libncurses6 -d libc6 --license MIT -f --deb-priority optional --deb-no-default-config-files bin/$(BIN)

$(RPMPATH): $(BINPATH)
	cd $(DIST) && fpm -s dir -t rpm --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d "ncurses >= 6" --license MIT -f bin/$(BIN)

$(PACPATH): $(BINPATH)
	cd $(DIST) && fpm -s dir -t pacman --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d "ncurses >= 6" --license MIT -f bin/$(BIN)

.PHONY: build
build: $(BINPATH)

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
	git tag v$(VERSION)
	git push --tags
	GITHUB_TOKEN=$(TOKEN) gothub release --user nytopop --repo askii --tag v$(VERSION)
	GITHUB_TOKEN=$(TOKEN) gothub upload --user nytopop --repo askii --tag v$(VERSION) --name $(BIN) --file $(BINPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload --user nytopop --repo askii --tag v$(VERSION) --name $(DEB) --file $(DEBPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload --user nytopop --repo askii --tag v$(VERSION) --name $(RPM) --file $(RPMPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload --user nytopop --repo askii --tag v$(VERSION) --name $(PAC) --file $(PACPATH)
