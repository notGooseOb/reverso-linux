PREFIX  ?= /usr/local
BINDIR  ?= $(PREFIX)/bin
CARGO   ?= cargo
BIN      = target/release/reverso-linux
DEST     = reverso-linux

all: $(BIN)
	cp -f $(BIN) $(DEST)

$(BIN): Cargo.toml build.rs $(wildcard src/*.rs)
	APP_VERSION="$(VERSION)" $(CARGO) build --release

install: all
	install -Dm755 $(DEST) $(DESTDIR)$(BINDIR)/reverso-linux

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/reverso-linux

clean:
	$(CARGO) clean
	rm -f $(DEST)

.PHONY: all install uninstall clean
