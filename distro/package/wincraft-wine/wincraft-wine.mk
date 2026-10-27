################################################################################
#
# wincraft-wine: Wine in WoW64 mode (x86-64 + x86 PE) for the WinCraft OS
# Derived from Buildroot's package/wine (GPL-2.0 build recipe).
#
################################################################################

WINCRAFT_WINE_VERSION = 11.0
WINCRAFT_WINE_SOURCE = wine-$(WINCRAFT_WINE_VERSION).tar.xz
WINCRAFT_WINE_SITE = https://dl.winehq.org/wine/source/11.0
WINCRAFT_WINE_LICENSE = LGPL-2.1+
WINCRAFT_WINE_LICENSE_FILES = COPYING.LIB LICENSE
WINCRAFT_WINE_DEPENDENCIES = host-bison host-flex host-wincraft-wine \
	xlib_libX11 xlib_libXext freetype fontconfig
HOST_WINCRAFT_WINE_DEPENDENCIES = host-bison host-flex host-freetype

# PE side: mingw-w64 from the build host (build/builder.sh installs it).
WINCRAFT_WINE_CONF_OPTS = \
	--with-wine-tools=../host-wincraft-wine-$(WINCRAFT_WINE_VERSION) \
	--enable-archs=i386,x86_64 \
	--with-mingw \
	--disable-tests \
	--enable-tools \
	--with-x \
	--with-freetype \
	--with-fontconfig \
	--with-xshape --with-xshm \
	--without-capi \
	--without-coreaudio \
	--without-gettext \
	--without-gettextpo \
	--without-gphoto \
	--without-opencl \
	--without-oss \
	--without-vulkan \
	--without-wayland \
	--without-cups \
	--without-dbus \
	--without-ffmpeg \
	--without-gstreamer \
	--without-krb5 \
	--without-netapi \
	--without-pcap \
	--without-pcsclite \
	--without-pulse \
	--without-sane \
	--without-sdl \
	--without-usb \
	--without-v4l2
WINCRAFT_WINE_CONF_ENV = FREETYPE_CONFIG=$(STAGING_DIR)/usr/bin/freetype-config

define WINCRAFT_WINE_OPTIONAL
$(if $(BR2_PACKAGE_$(1)),--with-$(2),--without-$(2))
endef
WINCRAFT_WINE_CONF_OPTS += \
	$(call WINCRAFT_WINE_OPTIONAL,ALSA_LIB,alsa) \
	$(call WINCRAFT_WINE_OPTIONAL,GNUTLS,gnutls) \
	$(call WINCRAFT_WINE_OPTIONAL,HAS_LIBGL,opengl) \
	$(call WINCRAFT_WINE_OPTIONAL,HAS_UDEV,udev) \
	$(call WINCRAFT_WINE_OPTIONAL,XLIB_LIBXCOMPOSITE,xcomposite) \
	$(call WINCRAFT_WINE_OPTIONAL,XLIB_LIBXCURSOR,xcursor) \
	$(call WINCRAFT_WINE_OPTIONAL,XLIB_LIBXFIXES,xfixes) \
	$(call WINCRAFT_WINE_OPTIONAL,XLIB_LIBXINERAMA,xinerama) \
	$(call WINCRAFT_WINE_OPTIONAL,XLIB_LIBXRANDR,xrandr) \
	$(call WINCRAFT_WINE_OPTIONAL,XLIB_LIBXRENDER,xrender) \
	$(call WINCRAFT_WINE_OPTIONAL,XLIB_LIBXXF86VM,xxf86vm)
WINCRAFT_WINE_CONF_OPTS += \
	$(if $(BR2_PACKAGE_XLIB_LIBXI),--with-xinput --with-xinput2,--without-xinput --without-xinput2)
WINCRAFT_WINE_DEPENDENCIES += \
	$(if $(BR2_PACKAGE_ALSA_LIB),alsa-lib) \
	$(if $(BR2_PACKAGE_GNUTLS),gnutls) \
	$(if $(BR2_PACKAGE_HAS_LIBGL),libgl) \
	$(if $(BR2_PACKAGE_HAS_UDEV),udev) \
	$(if $(BR2_PACKAGE_XLIB_LIBXCOMPOSITE),xlib_libXcomposite) \
	$(if $(BR2_PACKAGE_XLIB_LIBXCURSOR),xlib_libXcursor) \
	$(if $(BR2_PACKAGE_XLIB_LIBXFIXES),xlib_libXfixes) \
	$(if $(BR2_PACKAGE_XLIB_LIBXI),xlib_libXi) \
	$(if $(BR2_PACKAGE_XLIB_LIBXINERAMA),xlib_libXinerama) \
	$(if $(BR2_PACKAGE_XLIB_LIBXRANDR),xlib_libXrandr) \
	$(if $(BR2_PACKAGE_XLIB_LIBXRENDER),xlib_libXrender) \
	$(if $(BR2_PACKAGE_XLIB_LIBXXF86VM),xlib_libXxf86vm)

# Host side: only the cross tools (winebuild, widl, wrc, ...) are needed.
HOST_WINCRAFT_WINE_CONF_OPTS = \
	--enable-win64 \
	--disable-tests \
	--disable-win16 \
	--without-mingw \
	--without-x \
	--with-freetype \
	--without-alsa --without-capi --without-coreaudio --without-cups --without-dbus \
	--without-ffmpeg --without-fontconfig --without-gettext --without-gettextpo \
	--without-gphoto --without-gnutls --without-gssapi --without-gstreamer --without-krb5 \
	--without-netapi --without-opencl --without-opengl --without-osmesa --without-oss \
	--without-pcap --without-pcsclite --without-pulse --without-sane --without-sdl \
	--without-udev --without-usb --without-v4l2 --without-vulkan --without-wayland \
	--without-xcomposite --without-xcursor --without-xfixes --without-xinerama \
	--without-xinput --without-xinput2 --without-xrandr --without-xrender \
	--without-xshape --without-xshm --without-xxf86vm

define HOST_WINCRAFT_WINE_BUILD_CMDS
	$(HOST_MAKE_ENV) $(MAKE) -C $(@D) __tooldeps__
endef

# The target build uses the tools from the host build directory directly.
define HOST_WINCRAFT_WINE_INSTALL_CMDS
	:
endef

# Wine 11's install rules leave out the NLS tables (code pages, locales); without
# /usr/share/wine/nls every Wine process, wineboot included, fails at start.
define WINCRAFT_WINE_INSTALL_NLS
	mkdir -p $(TARGET_DIR)/usr/share/wine/nls
	$(INSTALL) -m 0644 $(@D)/nls/*.nls $(TARGET_DIR)/usr/share/wine/nls/
endef
WINCRAFT_WINE_POST_INSTALL_TARGET_HOOKS += WINCRAFT_WINE_INSTALL_NLS

$(eval $(autotools-package))
$(eval $(host-autotools-package))
