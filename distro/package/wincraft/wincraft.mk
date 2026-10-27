################################################################################
#
# wincraft: the Rust workspace of this repository
#
################################################################################

WINCRAFT_VERSION = 0.4.0
# build/build.sh stages only the Rust sources here (not dist/ or target/).
WINCRAFT_SITE = $(WINCRAFT_SOURCE_DIR)
WINCRAFT_SITE_METHOD = local
WINCRAFT_LICENSE = MIT
WINCRAFT_LICENSE_FILES = LICENSE
# libwayland-client/-egl and libEGL are loaded at run time; xkbcommon is linked (sctk).
WINCRAFT_DEPENDENCIES = libxkbcommon wayland mesa3d libgtk4 webkitgtk

WINCRAFT_CARGO = $(HOME)/.cargo/bin/cargo
WINCRAFT_CARGO_ENV = \
	CARGO_HOME=$(HOME)/.cargo \
	CARGO_TARGET_DIR=$(WINCRAFT_CARGO_TARGET_DIR) \
	RUSTUP_HOME=$(HOME)/.rustup \
	PATH=$(HOME)/.cargo/bin:$(BR_PATH) \
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$(TARGET_CC) \
	CC_x86_64_unknown_linux_gnu=$(TARGET_CC) \
	AR_x86_64_unknown_linux_gnu=$(TARGET_AR) \
	PKG_CONFIG=$(PKG_CONFIG_HOST_BINARY) \
	PKG_CONFIG_ALLOW_CROSS=1 \
	PKG_CONFIG_SYSROOT_DIR=$(STAGING_DIR) \
	PKG_CONFIG_LIBDIR=$(STAGING_DIR)/usr/lib/pkgconfig:$(STAGING_DIR)/usr/share/pkgconfig

WINCRAFT_GUEST_BINS = wincraft-init winlogon explorer notepad taskmgr start browser

define WINCRAFT_BUILD_CMDS
	cd $(@D) && $(WINCRAFT_CARGO_ENV) $(WINCRAFT_CARGO) build --release --locked --workspace \
		--target x86_64-unknown-linux-gnu \
		$(foreach b,$(WINCRAFT_GUEST_BINS),--bin $(b))
	cd $(@D) && $(WINCRAFT_CARGO_ENV) $(WINCRAFT_CARGO) build --release --locked \
		--target x86_64-unknown-linux-musl -p wincraft-boot
endef

define WINCRAFT_INSTALL_TARGET_CMDS
	$(foreach b,$(WINCRAFT_GUEST_BINS),
		$(INSTALL) -D -m 0755 $(WINCRAFT_CARGO_TARGET_DIR)/x86_64-unknown-linux-gnu/release/$(b) \
			$(TARGET_DIR)/usr/bin/$(b)
	)
	$(INSTALL) -D -m 0755 $(WINCRAFT_CARGO_TARGET_DIR)/x86_64-unknown-linux-musl/release/wincraft-boot \
		$(BINARIES_DIR)/wincraft-boot
endef

$(eval $(generic-package))
