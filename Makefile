BUILD_DIR = a2vm-macos/build

SWIFT_SOURCES = \
	a2vm-macos/main.swift \
	a2vm-macos/EmulatorController.swift \
	a2vm-macos/EmulatorView.swift \
	a2vm-macos/MetalRenderer.swift \
	a2vm-macos/KeyMapper.swift

.PHONY: all rust-ffi macos-shaders macos-app run-app clean-macos

all: macos-app

rust-ffi:
	cargo build --release -p a2vm-ffi

macos-shaders:
	@mkdir -p $(BUILD_DIR)
	xcrun metal -c -o $(BUILD_DIR)/Shaders.air a2vm-macos/Shaders.metal
	xcrun metallib -o $(BUILD_DIR)/Shaders.metallib $(BUILD_DIR)/Shaders.air

macos-app: rust-ffi macos-shaders
	@mkdir -p $(BUILD_DIR)/A2VM.app/Contents/MacOS
	@mkdir -p $(BUILD_DIR)/A2VM.app/Contents/Resources
	swiftc -O \
		$(SWIFT_SOURCES) \
		-import-objc-header a2vm-macos/a2vm-ffi-Bridging.h \
		-L target/release -la2vm_ffi \
		-framework AppKit -framework Metal -framework MetalKit \
		-framework AudioToolbox -framework CoreAudio \
		-o $(BUILD_DIR)/A2VM.app/Contents/MacOS/A2VM
	cp $(BUILD_DIR)/Shaders.metallib $(BUILD_DIR)/A2VM.app/Contents/Resources/
	cp a2vm-macos/Info.plist $(BUILD_DIR)/A2VM.app/Contents/

run-app: macos-app
	open $(BUILD_DIR)/A2VM.app

clean-macos:
	rm -rf $(BUILD_DIR)
