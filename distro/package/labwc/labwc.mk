################################################################################
#
# labwc: Wayland compositor and window manager (wlroots), the WinCraft display
# server (ADR-018). Openbox-compatible configuration and themes.
#
################################################################################

# 0.9.x is the maintenance branch for wlroots 0.19 (Buildroot 2026.02 ships 0.19.2).
LABWC_VERSION = 0.9.8
LABWC_SITE = $(call github,labwc,labwc,$(LABWC_VERSION))
LABWC_LICENSE = GPL-2.0
LABWC_LICENSE_FILES = LICENSE
LABWC_DEPENDENCIES = host-pkgconf host-wayland wlroots wayland wayland-protocols \
	libxkbcommon libdrm libxml2 libglib2 cairo pango libinput pixman libpng
# Window icons need libsfdo (not packaged); WinCraft's taskbar draws its own icons.
LABWC_CONF_OPTS = \
	-Dman-pages=disabled \
	-Dsvg=disabled \
	-Dicon=disabled \
	-Dnls=disabled \
	-Dtest=disabled \
	-Dstatic_analyzer=disabled

ifeq ($(BR2_PACKAGE_WLROOTS_XWAYLAND),y)
LABWC_CONF_OPTS += -Dxwayland=enabled
LABWC_DEPENDENCIES += libxcb xcb-util-wm xwayland
else
LABWC_CONF_OPTS += -Dxwayland=disabled
endif

$(eval $(meson-package))
