.PHONY: all build release clean install uninstall install-user uninstall-user

BINARY_NAME=memwatchdog
PREFIX?=/usr/local
BINDIR=$(PREFIX)/bin
SYSTEMD_SYSTEM_DIR=/etc/systemd/system

USER_BINDIR=$(HOME)/.local/bin
USER_SYSTEMD_DIR=$(HOME)/.config/systemd/user

all: release

build:
	rustc -O src/main.rs -o $(BINARY_NAME)

release:
	@if command -v cargo >/dev/null 2>&1; then \
		cargo build --release && cp target/release/$(BINARY_NAME) . ; \
	else \
		rustc -O src/main.rs -o $(BINARY_NAME) ; \
	fi

clean:
	rm -f $(BINARY_NAME)
	rm -rf target

# System-wide installation (requires sudo)
# After running 'sudo make install', you can safely delete the source code folder.
install: release
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 $(BINARY_NAME) $(DESTDIR)$(BINDIR)/$(BINARY_NAME)
	@if [ -d $(SYSTEMD_SYSTEM_DIR) ]; then \
		install -d $(DESTDIR)$(SYSTEMD_SYSTEM_DIR) ; \
		echo "[Unit]" > $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "Description=Low Latency Memory Watchdog Daemon (Rust)" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "After=multi-user.target" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "[Service]" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "Type=simple" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "ExecStart=$(BINDIR)/$(BINARY_NAME) --threshold 200 --interval 200 --grace 1000" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "Restart=always" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "RestartSec=2" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "StandardOutput=journal" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "StandardError=journal" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "[Install]" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		echo "WantedBy=multi-user.target" >> $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service ; \
		systemctl daemon-reload 2>/dev/null || true ; \
		echo "Installed system service to $(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service" ; \
	fi
	@echo ""
	@echo "Successfully installed $(BINARY_NAME) to $(BINDIR)/$(BINARY_NAME)."
	@echo "You can now safely remove this source directory if desired."
	@echo "To enable and start the system service, run:"
	@echo "  sudo systemctl enable --now $(BINARY_NAME)"

uninstall:
	systemctl stop $(BINARY_NAME) 2>/dev/null || true
	systemctl disable $(BINARY_NAME) 2>/dev/null || true
	rm -f $(DESTDIR)$(BINDIR)/$(BINARY_NAME)
	rm -f $(DESTDIR)$(SYSTEMD_SYSTEM_DIR)/$(BINARY_NAME).service
	systemctl daemon-reload 2>/dev/null || true
	@echo "Uninstalled $(BINARY_NAME)."

# User-level installation (no sudo required)
install-user: release
	install -d $(USER_BINDIR)
	install -m 755 $(BINARY_NAME) $(USER_BINDIR)/$(BINARY_NAME)
	install -d $(USER_SYSTEMD_DIR)
	@echo "[Unit]" > $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "Description=Low Latency Memory Watchdog Daemon (Rust)" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "After=default.target" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "[Service]" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "Type=simple" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "ExecStart=$(USER_BINDIR)/$(BINARY_NAME) --threshold 200 --interval 200 --grace 1000 --notify" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "Restart=always" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "RestartSec=2" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "StandardOutput=journal" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "StandardError=journal" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "[Install]" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	@echo "WantedBy=default.target" >> $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	systemctl --user daemon-reload 2>/dev/null || true
	@echo ""
	@echo "Successfully installed $(BINARY_NAME) to $(USER_BINDIR)/$(BINARY_NAME)."
	@echo "You can now safely remove this source directory if desired."
	@echo "To enable and start the user service, run:"
	@echo "  systemctl --user enable --now $(BINARY_NAME)"

uninstall-user:
	systemctl --user stop $(BINARY_NAME) 2>/dev/null || true
	systemctl --user disable $(BINARY_NAME) 2>/dev/null || true
	rm -f $(USER_BINDIR)/$(BINARY_NAME)
	rm -f $(USER_SYSTEMD_DIR)/$(BINARY_NAME).service
	systemctl --user daemon-reload 2>/dev/null || true
	@echo "Uninstalled $(BINARY_NAME) user service."
