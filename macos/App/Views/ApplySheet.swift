import OmarchyThemesKit
import OmarchyThemesStores
import SwiftUI

/// A downloaded theme to show the Apply sheet for, with the wallpaper to preselect.
struct ApplyTarget: Identifiable {
    let id = UUID()
    let theme: InstalledTheme
    let wallpaperFile: String?
}

/// What the Apply sheet returns: the per-aspect choices, and whether to make them the one-click
/// defaults (`DesktopStore.apply` saves them; the sheet doesn't write settings itself).
struct ApplyChoices {
    var options: ApplyOptions
    var remember: Bool
}

/// Per-aspect Apply choices. macOS only lets apps set the wallpaper, so light/dark and accent are
/// shown disabled with the reason (and a way to change them in System Settings).
struct ApplySheet: View {
    @Environment(DesktopStore.self) private var desktop
    @Environment(Preferences.self) private var preferences
    @Environment(\.dismiss) private var dismiss

    let theme: InstalledTheme
    let wallpaperFile: String?
    let onApply: (ApplyChoices) -> Void

    @State private var setWallpaper = true
    @State private var fit = WallpaperFit.fill
    @State private var remember = false
    @State private var loaded = false

    private var canWallpaper: Bool { desktop.capabilities.contains(.wallpaper) && !theme.wallpapers.isEmpty }
    private var wallpaperURL: URL? { (wallpaperFile ?? theme.wallpapers.first).map(theme.wallpaperURL) }

    /// Light/dark and accent keep their saved defaults (they're reported as not supported), so
    /// remembering choices here doesn't rewrite them.
    private var options: ApplyOptions {
        var options = preferences.settings.applyDefaults
        options.wallpaper = canWallpaper && setWallpaper
        options.fit = fit
        return options
    }

    var body: some View {
        VStack(spacing: 0) {
            Form {
                wallpaperSection
                appearanceSection
                Section {
                    Toggle("Use these choices for one-click Apply", isOn: $remember)
                } footer: {
                    Text(desktop.hasOriginalSnapshot
                         ? "You can go back to your original desktop any time from Settings."
                         : "Your current desktop picture is saved first, so you can restore it from Settings.")
                        .foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            .scrollDisabled(true)

            Divider()
            buttons
        }
        .frame(width: 500)
        .navigationTitle("Apply \(theme.name)")
        .onAppear {
            guard !loaded else { return }
            loaded = true
            setWallpaper = preferences.settings.applyDefaults.wallpaper
            fit = desktop.defaultFit
        }
    }

    private var wallpaperSection: some View {
        Section {
            if canWallpaper {
                HStack(alignment: .top, spacing: 14) {
                    ThumbnailImage(url: wallpaperURL, maxPixelSize: 400)
                        .frame(width: 128, height: 72)
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.primary.opacity(0.12)))
                    VStack(alignment: .leading, spacing: 8) {
                        Toggle("Set as desktop picture", isOn: $setWallpaper)
                            .accessibilityIdentifier("applyWallpaperToggle")
                        Picker("Fit", selection: $fit) {
                            ForEach(desktop.supportedFits) { Text($0.displayName).tag($0) }
                        }
                        // No .fixedSize(): in a grouped Form it widens the row past the
                        // sheet, and the whole form is then cropped on both sides.
                        .disabled(!setWallpaper)
                    }
                }
            } else {
                Text(theme.wallpapers.isEmpty ? "This theme has no wallpaper." : "This Mac can't set the wallpaper.")
                    .foregroundStyle(.secondary)
            }
        } header: {
            Text("Wallpaper")
        } footer: {
            if canWallpaper {
                Text("Applies to every display, on the current Space.")
                    .foregroundStyle(.secondary)
            }
        }
    }

    /// Shown for completeness, always off: macOS has no API for apps to change these.
    private var appearanceSection: some View {
        Section {
            Toggle(theme.mode == .light ? "Switch to Light appearance" : "Switch to Dark appearance", isOn: .constant(false))
                .disabled(true)
            Toggle(isOn: .constant(false)) {
                HStack(spacing: 6) {
                    if let accent = theme.palette?.accent {
                        Circle().fill(Color(accent)).frame(width: 12, height: 12)
                        Text("Use the accent color (\(accent.hex.uppercased()))")
                    } else {
                        Text("Use the theme's accent color")
                    }
                }
            }
            .disabled(true)
        } header: {
            Text("Appearance")
        } footer: {
            VStack(alignment: .leading, spacing: 4) {
                Text("macOS doesn't let apps change the appearance or accent color. You can set them yourself in System Settings.")
                OpenAppearanceSettingsButton()
            }
            .foregroundStyle(.secondary)
        }
    }

    private var buttons: some View {
        HStack {
            Spacer()
            Button("Cancel", role: .cancel) { dismiss() }
                .keyboardShortcut(.cancelAction)
                .accessibilityIdentifier("applySheetCancel")
            Button("Apply") {
                let choices = ApplyChoices(options: options, remember: remember)
                dismiss()
                onApply(choices)
            }
            .keyboardShortcut(.defaultAction)
            .disabled(!(canWallpaper && setWallpaper))
            .accessibilityIdentifier("applySheetConfirm")
        }
        .padding(16)
    }
}

struct OpenAppearanceSettingsButton: View {
    var body: some View {
        Button("Open Appearance Settings") {
            NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.Appearance-Settings.extension")!)
        }
        .buttonStyle(.link)
    }
}
