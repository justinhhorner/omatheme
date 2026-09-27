import OmarchyThemesKit
import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            GeneralSettingsView()
                .tabItem { Label("General", systemImage: "gearshape") }
            StorageSettingsView()
                .tabItem { Label("Storage", systemImage: "internaldrive") }
        }
        .frame(width: 540)
    }
}

struct GeneralSettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow
    @State private var confirmRestore = false
    @State private var isRestoring = false

    private var wallpaperDefault: Binding<Bool> {
        Binding(get: { model.settings.applyDefaults.wallpaper }, set: { value in model.updateSettings { $0.applyDefaults.wallpaper = value } })
    }

    private var fitDefault: Binding<WallpaperFit> {
        Binding(get: { model.supportedFits.contains(model.settings.applyDefaults.fit) ? model.settings.applyDefaults.fit : .fill },
                set: { value in model.updateSettings { $0.applyDefaults.fit = value } })
    }

    var body: some View {
        Form {
            Section {
                Toggle("Set the desktop picture", isOn: wallpaperDefault)
                Picker("Fit", selection: fitDefault) {
                    ForEach(model.supportedFits) { Text($0.displayName).tag($0) }
                }
                .disabled(!model.settings.applyDefaults.wallpaper)
                LabeledContent("Light/dark appearance", value: "Not available on macOS")
                LabeledContent("Accent color", value: "Not available on macOS")
            } header: {
                Text("One-Click Apply")
            } footer: {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Used by Apply in Downloaded. macOS doesn't let apps change the appearance or accent color.")
                    OpenAppearanceSettingsButton()
                }
                .foregroundStyle(.secondary)
            }

            Section {
                Text(model.hasOriginalSnapshot
                     ? "Puts back the desktop picture you had before applying your first theme."
                     : "Your desktop picture is saved automatically the first time you apply a theme. Nothing has been changed yet.")
                    .foregroundStyle(.secondary)
                HStack {
                    Button("Restore My Original Desktop…") { confirmRestore = true }
                        .disabled(!model.hasOriginalSnapshot || isRestoring)
                        .accessibilityIdentifier("restoreOriginalButton")
                    if isRestoring { ProgressView().controlSize(.small) }
                }
                ContextBanner(context: .settings)
            } header: {
                Text("Original Desktop")
            }

            Section {
                Button("Show Welcome Screen") {
                    model.showWelcome = true
                    openWindow(id: "main")
                }
            }
        }
        .formStyle(.grouped)
        .confirmationDialog("Restore your original desktop?", isPresented: $confirmRestore) {
            Button("Restore") {
                Task {
                    isRestoring = true
                    model.banners[.settings] = await model.restoreOriginalDesktop()
                    isRestoring = false
                }
            }
        } message: {
            Text("Your desktop picture goes back to the one you had before applying your first theme.")
        }
    }
}

struct StorageSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var themesSize: Int64?
    @State private var cacheSize: Int64?
    @State private var result: Banner?

    var body: some View {
        Form {
            Section("Downloaded Themes") {
                LabeledContent(model.installed.count == 1 ? "1 theme" : "\(model.installed.count) themes",
                               value: themesSize.map { $0.formatted(.byteCount(style: .file)) } ?? "…")
                Button("Show in Finder") { NSWorkspace.shared.activateFileViewerSelecting([model.paths.themesDir]) }
            }

            Section {
                LabeledContent("Catalog, GitHub responses and images", value: cacheSize.map { $0.formatted(.byteCount(style: .file)) } ?? "…")
                Button("Clear Cache") {
                    do {
                        try model.clearCache()
                        result = Banner(kind: .success, title: "Cache cleared",
                                        message: "The catalog and theme details will be downloaded again when needed. Downloaded themes were kept.")
                    } catch {
                        result = Banner(kind: .error, title: "Couldn't clear the cache", message: error.localizedDescription)
                    }
                    Task { await measure() }
                }
                if let result {
                    BannerView(banner: result) { self.result = nil }
                }
            } header: {
                Text("Cache")
            }

            Section("GitHub") {
                Text(model.gitHubTokenIsSet
                     ? "Using the token from the GITHUB_TOKEN environment variable for GitHub requests."
                     : "Opening a theme uses GitHub's public API, which allows 60 theme lookups an hour. Revisiting a theme is usually free. Launch the app with a GITHUB_TOKEN environment variable to raise the limit.")
                    .foregroundStyle(.secondary)
            }

            Section {
                LabeledContent("Data folder") {
                    Text((model.paths.root.path as NSString).abbreviatingWithTildeInPath)
                        .textSelection(.enabled)
                        .truncationMode(.middle)
                }
            }
        }
        .formStyle(.grouped)
        .task { await measure() }
    }

    private func measure() async {
        let paths = model.paths
        (themesSize, cacheSize) = await Task.detached {
            (Self.size(of: paths.themesDir), Self.size(of: paths.cacheDir))
        }.value
    }

    private nonisolated static func size(of directory: URL) -> Int64 {
        let keys: [URLResourceKey] = [.totalFileAllocatedSizeKey, .isRegularFileKey]
        guard let enumerator = FileManager.default.enumerator(at: directory, includingPropertiesForKeys: keys) else { return 0 }
        var total: Int64 = 0
        for case let url as URL in enumerator {
            guard let values = try? url.resourceValues(forKeys: Set(keys)), values.isRegularFile == true else { continue }
            total += Int64(values.totalFileAllocatedSize ?? 0)
        }
        return total
    }
}
