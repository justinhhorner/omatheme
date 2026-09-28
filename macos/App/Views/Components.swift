import AppKit
import OmarchyThemesKit
import OmarchyThemesStores
import SwiftUI

extension Color {
    init(_ rgb: RgbColor) {
        let c = rgb.unitComponents
        self.init(.sRGB, red: c.red, green: c.green, blue: c.blue)
    }
}

extension SummaryKind {
    var symbol: String {
        switch self {
        case .success: "checkmark.circle.fill"
        case .info: "info.circle.fill"
        case .warning: "exclamationmark.triangle.fill"
        case .error: "xmark.octagon.fill"
        }
    }

    var tint: Color {
        switch self {
        case .success: .green
        case .info: .accentColor
        case .warning: .orange
        case .error: .red
        }
    }
}

// MARK: Banners

/// An inline result or notice, dismissable, like an InfoBar.
struct BannerView: View {
    let banner: Banner
    var onDismiss: (() -> Void)?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: banner.kind.symbol)
                .foregroundStyle(banner.kind.tint)
            VStack(alignment: .leading, spacing: 2) {
                Text(banner.title).fontWeight(.semibold)
                if !banner.message.isEmpty {
                    Text(banner.message)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 0)
            if let onDismiss {
                Button(action: onDismiss) {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
                .accessibilityLabel("Dismiss")
            }
        }
        .padding(12)
        .background(banner.kind.tint.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(banner.kind.tint.opacity(0.25)))
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("banner")
    }
}

/// The banner stored for `context`, if any.
struct ContextBanner: View {
    @Environment(Banners.self) private var banners
    let context: BannerContext

    var body: some View {
        if let banner = banners[context] {
            BannerView(banner: banner) { banners[context] = nil }
                .transition(.move(edge: .top).combined(with: .opacity))
        }
    }
}

// MARK: Badges

struct ModeBadge: View {
    let mode: AppearanceMode

    var body: some View {
        Label(mode == .light ? "Light" : "Dark", systemImage: mode == .light ? "sun.max.fill" : "moon.fill")
            .font(.caption.weight(.medium))
            .padding(.horizontal, 7)
            .padding(.vertical, 2)
            .background(.quaternary, in: Capsule())
            .accessibilityLabel(mode == .light ? "Light theme" : "Dark theme")
    }
}

struct StatusBadge: View {
    let title: String
    let symbol: String
    var tint: Color = .accentColor

    /// The theme is on the desktop.
    static var applied: StatusBadge {
        StatusBadge(title: "Applied", symbol: "checkmark.circle.fill", tint: .green)
    }

    static var downloaded: StatusBadge {
        StatusBadge(title: "Downloaded", symbol: "arrow.down.circle.fill")
    }

    var body: some View {
        Label(title, systemImage: symbol)
            .font(.caption.weight(.semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 7)
            .padding(.vertical, 3)
            .background(tint.gradient, in: Capsule())
            .shadow(color: .black.opacity(0.25), radius: 2, y: 1)
    }
}

// MARK: Colors

/// A named color: swatch plus label and hex. Click to copy the hex value.
struct ColorChip: View {
    let name: String
    let color: RgbColor
    @State private var copied = false

    var body: some View {
        Button {
            Pasteboard.copy(color.hex)
            copied = true
            Task {
                try? await Task.sleep(for: .seconds(1.2))
                copied = false
            }
        } label: {
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color(color))
                    .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.primary.opacity(0.15)))
                    .frame(width: 32, height: 32)
                VStack(alignment: .leading, spacing: 1) {
                    Text(name).font(.callout)
                    Text(copied ? "Copied" : color.hex.uppercased())
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("Copy \(color.hex.uppercased())")
        .accessibilityLabel("\(name), \(color.hex.uppercased())")
    }
}

/// A small terminal-color square with its name in the tooltip.
struct SwatchSquare: View {
    let swatch: NamedColor

    var body: some View {
        RoundedRectangle(cornerRadius: 5)
            .fill(Color(swatch.color))
            .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(.primary.opacity(0.12)))
            .frame(height: 28)
            .help("\(swatch.name) \(swatch.color.hex.uppercased())")
            .accessibilityLabel("\(swatch.name), \(swatch.color.hex.uppercased())")
    }
}

// MARK: Shared actions

struct ShowInFinderButton: View {
    let url: URL

    var body: some View {
        Button("Show in Finder") { NSWorkspace.shared.activateFileViewerSelecting([url]) }
    }
}

/// The message of the "Remove <theme>?" confirmation.
struct RemoveDownloadMessage: View {
    /// Whether the theme is the one on the desktop.
    let isOnDesktop: Bool

    var body: some View {
        Text(isOnDesktop
            ? "Its wallpapers are deleted from this Mac, including the one on your desktop. You can download the theme again at any time."
            : "Its wallpapers are deleted from this Mac. You can download the theme again at any time.")
    }
}

enum Pasteboard {
    static func copy(_ string: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(string, forType: .string)
    }
}
