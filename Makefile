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
OSXPATH=$(DIST)/osx/$(BIN)
WINPATH=$(DIST)/win/$(BIN).exe

.PHONY: all
all: $(BINPATH) $(DEBPATH) $(RPMPATH) $(PACPATH)

$(BINPATH):
	cargo build --release
	mkdir -p $(DIST)/bin
	cp target/release/$(BIN) $(BINPATH)

$(DEBPATH): $(BINPATH)
	cd $(DIST) && fpm -s dir -t deb --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d "libxcb1" -d "libxcb-render0" -d "libxcb-shape0" -d "libxcb-xfixes0" -d "libxau6" -d "libxdmcp6" -d libc6 --license MIT -f --deb-priority optional --deb-no-default-config-files bin/$(BIN)

$(RPMPATH): $(BINPATH)
	cd $(DIST) && fpm -s dir -t rpm --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d "libxcb >= 1" --license MIT -f bin/$(BIN)

$(PACPATH): $(BINPATH)
	cd $(DIST) && fpm -s dir -t pacman --prefix /usr -n $(NAME) -v $(VERSION) --description $(DESCRIPTION) --maintainer $(AUTHOR) --vendor $(AUTHOR) -d "libxcb" --license MIT -f bin/$(BIN)

OSX_PREFIX=/usr/local/osx-ndk-x86

$(OSXPATH):
	mkdir -p $(DIST)/osx
	PKG_CONFIG_ALLOW_CROSS=1 PATH=$(OSX_PREFIX)/bin:$$PATH LD_LIBRARY_PATH=$(OSX_PREFIX)/lib cargo build --target=x86_64-apple-darwin --release
	cp target/x86_64-apple-darwin/release/$(BIN) $(OSXPATH)

$(WINPATH):
	mkdir -p $(DIST)/win
	cargo build --target=x86_64-pc-windows-gnu --release
	cp target/x86_64-pc-windows-gnu/release/$(BIN).exe $(WINPATH)

.PHONY: cross
cross: $(OSXPATH) $(WINPATH)

.PHONY: everything
everything: all cross

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

CHANGELOG=$(DIST)/changelog
TAG=v$(VERSION)

.PHONY: release
release: distclean everything
	$(eval TOKEN := $(shell cat ~/.github-token-askii))
	git log $(shell git describe --tags --abbrev=0)..HEAD --oneline > $(CHANGELOG)
	cargo publish
	git tag $(TAG)
	git push --tags
	GITHUB_TOKEN=$(TOKEN) TAG=$(TAG) CHANGELOG=$(CHANGELOG) ./release.sh
	GITHUB_TOKEN=$(TOKEN) gothub upload -u nytopop -r askii -t $(TAG) -n $(BIN) -f $(BINPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload -u nytopop -r askii -t $(TAG) -n $(DEB) -f $(DEBPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload -u nytopop -r askii -t $(TAG) -n $(RPM) -f $(RPMPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload -u nytopop -r askii -t $(TAG) -n $(PAC) -f $(PACPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload -u nytopop -r askii -t $(TAG) -n $(BIN)-osx -f $(OSXPATH)
	GITHUB_TOKEN=$(TOKEN) gothub upload -u nytopop -r askii -t $(TAG) -n $(BIN).exe -f $(WINPATH)
